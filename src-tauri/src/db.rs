//! SQLite storage (doc-2 §1.5): schema in `migrations/`, query functions here.
//! Times are milliseconds (absolute since the Unix epoch, or relative to a meeting's start).

use std::path::Path;
use std::str::FromStr;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("settings value: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Which of the three transcript passes produced a segment (doc-2 §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pass {
    Caption,
    Rolling,
    Final,
}

impl Pass {
    fn as_str(self) -> &'static str {
        match self {
            Pass::Caption => "caption",
            Pass::Rolling => "rolling",
            Pass::Final => "final",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct Meeting {
    pub id: i64,
    pub title: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub status: String,
    pub dir: String,
    /// The app died during this recording and it was recovered (VRT-41).
    pub interrupted: bool,
    /// Recorded in a room: everyone there is on the mic (VRT-47); otherwise a call (mic = me).
    pub in_room: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct Track {
    pub id: i64,
    pub meeting_id: i64,
    pub source_kind: String,
    pub source_id: String,
    pub device_name: String,
    pub path: String,
    pub seconds: f64,
}

#[derive(Debug, Clone)]
pub struct NewTrack<'a> {
    pub source_kind: &'a str,
    pub source_id: &'a str,
    pub device_name: &'a str,
    pub path: &'a str,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct Segment {
    pub id: i64,
    pub meeting_id: i64,
    pub track_id: Option<i64>,
    pub pass: String,
    pub speaker_id: Option<i64>,
    pub speaker_label: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words_json: Option<String>,
}

/// A meeting's folder on the Nextcloud server (VRT-39).
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct SyncMeeting {
    pub meeting_id: i64,
    pub remote_dir: String,
    /// "phone" (came in through the inbox) or "desktop".
    pub origin: String,
    pub transcript_hash: Option<String>,
    pub mix_uploaded: bool,
    pub gone: bool,
}

/// A recording found in the Nextcloud inbox (VRT-39).
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct InboxItem {
    pub remote_path: String,
    pub etag: String,
    pub meeting_id: Option<i64>,
    /// "importing", "done" or "failed".
    pub state: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewSegment {
    pub track_id: Option<i64>,
    pub speaker_id: Option<i64>,
    pub speaker_label: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct SearchHit {
    pub meeting_id: i64,
    pub segment_id: i64,
    pub start_ms: i64,
    /// Matching words wrapped in U+0001 and U+0002 (text never contains them; brackets can).
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct Speaker {
    pub id: i64,
    pub name: String,
    pub color: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSample {
    pub id: i64,
    pub speaker_id: i64,
    pub embedding: Vec<f32>,
    pub clean_seconds: f64,
}

/// A meeting's speaker label with its voice and who it is (table meeting_voices, VRT-18).
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingVoice {
    pub label: String,
    pub speaker_id: Option<i64>,
    pub suggested_id: Option<i64>,
    pub distance: Option<f64>,
    pub embedding: Option<Vec<f32>>,
    pub encoder_id: Option<String>,
    pub clean_seconds: f64,
    /// The user named or confirmed it (matching leaves it alone).
    pub confirmed: bool,
}

/// A person on the Speakers screen.
#[derive(Debug, Clone, PartialEq, Serialize, FromRow)]
pub struct Person {
    pub id: i64,
    pub name: String,
    pub color: i64,
    pub meetings: i64,
    pub samples: i64,
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Open (or create) the database file and bring the schema up to date.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        Self::with_pool(pool).await
    }

    /// A private in-memory database (tests). One connection, because each in-memory
    /// connection is its own database.
    pub async fn open_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::with_pool(pool).await
    }

    async fn with_pool(pool: SqlitePool) -> Result<Self> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    // ---------------------------------------------------------------- meetings

    pub async fn create_meeting(&self, title: &str, started_at: i64, dir: &str) -> Result<i64> {
        let id = sqlx::query("INSERT INTO meetings (title, started_at, dir) VALUES (?, ?, ?)")
            .bind(title)
            .bind(started_at)
            .bind(dir)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
        Ok(id)
    }

    pub async fn finish_meeting(&self, id: i64, duration_ms: i64, status: &str) -> Result<()> {
        sqlx::query("UPDATE meetings SET duration_ms = ?, status = ? WHERE id = ?")
            .bind(duration_ms)
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Nextcloud sync (VRT-39): a meeting's folder on the server.
    pub async fn sync_meeting(&self, meeting_id: i64) -> Result<Option<SyncMeeting>> {
        Ok(
            sqlx::query_as("SELECT * FROM sync_meetings WHERE meeting_id = ?")
                .bind(meeting_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn sync_meetings(&self) -> Result<Vec<SyncMeeting>> {
        Ok(
            sqlx::query_as("SELECT * FROM sync_meetings ORDER BY meeting_id")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn add_sync_meeting(
        &self,
        meeting_id: i64,
        remote_dir: &str,
        origin: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_meetings (meeting_id, remote_dir, origin) VALUES (?, ?, ?)
             ON CONFLICT (meeting_id) DO UPDATE SET remote_dir = excluded.remote_dir, gone = 0",
        )
        .bind(meeting_id)
        .bind(remote_dir)
        .bind(origin)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_sync_uploaded(
        &self,
        meeting_id: i64,
        transcript_hash: Option<&str>,
        mix_uploaded: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sync_meetings SET transcript_hash = ?, mix_uploaded = ? WHERE meeting_id = ?",
        )
        .bind(transcript_hash)
        .bind(mix_uploaded)
        .bind(meeting_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_sync_gone(&self, meeting_id: i64) -> Result<()> {
        sqlx::query("UPDATE sync_meetings SET gone = 1 WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Forget all sync state (disconnect).
    pub async fn clear_sync(&self) -> Result<()> {
        sqlx::query("DELETE FROM sync_meetings")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM sync_inbox")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn inbox_items(&self) -> Result<Vec<InboxItem>> {
        Ok(
            sqlx::query_as("SELECT * FROM sync_inbox ORDER BY remote_path")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn set_inbox_item(
        &self,
        remote_path: &str,
        etag: &str,
        meeting_id: Option<i64>,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_inbox (remote_path, etag, meeting_id, state, error) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (remote_path) DO UPDATE SET etag = excluded.etag,
                 meeting_id = excluded.meeting_id, state = excluded.state, error = excluded.error",
        )
        .bind(remote_path)
        .bind(etag)
        .bind(meeting_id)
        .bind(state)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_inbox_item(&self, remote_path: &str) -> Result<()> {
        sqlx::query("DELETE FROM sync_inbox WHERE remote_path = ?")
            .bind(remote_path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Whether the meeting was in a room (several people on the mic) or a call (VRT-47).
    pub async fn set_in_room(&self, id: i64, in_room: bool) -> Result<()> {
        sqlx::query("UPDATE meetings SET in_room = ? WHERE id = ?")
            .bind(in_room)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark a recording as cut off by a crash and recovered (VRT-41).
    pub async fn set_interrupted(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE meetings SET interrupted = 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Remove a track that holds nothing usable (VRT-41).
    pub async fn delete_track(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM tracks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A track file moved (Settings > Storage, VRT-27).
    pub async fn set_track_path(&self, id: i64, path: &str) -> Result<()> {
        sqlx::query("UPDATE tracks SET path = ? WHERE id = ?")
            .bind(path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_meeting_dir(&self, id: i64, dir: &str) -> Result<()> {
        sqlx::query("UPDATE meetings SET dir = ? WHERE id = ?")
            .bind(dir)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn rename_meeting(&self, id: i64, title: &str) -> Result<()> {
        sqlx::query("UPDATE meetings SET title = ? WHERE id = ?")
            .bind(title)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Newest first.
    pub async fn list_meetings(&self) -> Result<Vec<Meeting>> {
        Ok(
            sqlx::query_as("SELECT * FROM meetings ORDER BY started_at DESC, id DESC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn get_meeting(&self, id: i64) -> Result<Option<Meeting>> {
        Ok(sqlx::query_as("SELECT * FROM meetings WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Delete a meeting with its tracks and segments. Returns false if it didn't exist.
    pub async fn delete_meeting(&self, id: i64) -> Result<bool> {
        let n = sqlx::query("DELETE FROM meetings WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n > 0)
    }

    /// Named speakers per meeting (distinct, by name), for the Library's speaker avatars.
    pub async fn meeting_speakers(&self) -> Result<Vec<(i64, Speaker)>> {
        let rows: Vec<(i64, i64, String, i64)> = sqlx::query_as(
            "SELECT DISTINCT s.meeting_id, sp.id, sp.name, sp.color
             FROM segments s JOIN speakers sp ON sp.id = s.speaker_id
             ORDER BY s.meeting_id, sp.name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(m, id, name, color)| (m, Speaker { id, name, color }))
            .collect())
    }

    // ---------------------------------------------------------------- tracks

    pub async fn add_track(&self, meeting_id: i64, t: &NewTrack<'_>) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO tracks (meeting_id, source_kind, source_id, device_name, path, seconds)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(meeting_id)
        .bind(t.source_kind)
        .bind(t.source_id)
        .bind(t.device_name)
        .bind(t.path)
        .bind(t.seconds)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// Record a track's final length (tracks are added when recording starts).
    pub async fn set_track_seconds(&self, id: i64, seconds: f64) -> Result<()> {
        sqlx::query("UPDATE tracks SET seconds = ? WHERE id = ?")
            .bind(seconds)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Store where a recording lost audio while the computer slept (VRT-36).
    pub async fn add_gaps(&self, meeting_id: i64, gaps: &[(i64, i64)]) -> Result<()> {
        for (at_ms, missing_ms) in gaps {
            sqlx::query(
                "INSERT INTO meeting_gaps (meeting_id, at_ms, missing_ms) VALUES (?, ?, ?)",
            )
            .bind(meeting_id)
            .bind(at_ms)
            .bind(missing_ms)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// A meeting's gaps as (at_ms, missing_ms), in order.
    pub async fn list_gaps(&self, meeting_id: i64) -> Result<Vec<(i64, i64)>> {
        Ok(sqlx::query_as(
            "SELECT at_ms, missing_ms FROM meeting_gaps WHERE meeting_id = ? ORDER BY at_ms",
        )
        .bind(meeting_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_tracks(&self, meeting_id: i64) -> Result<Vec<Track>> {
        Ok(
            sqlx::query_as("SELECT * FROM tracks WHERE meeting_id = ? ORDER BY id")
                .bind(meeting_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    // ---------------------------------------------------------------- segments

    pub async fn insert_segments(
        &self,
        meeting_id: i64,
        pass: Pass,
        segments: &[NewSegment],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        insert_segments_tx(&mut tx, meeting_id, pass, segments).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Replace the segments of `replaced` passes that start within `[from_ms, to_ms)` by
    /// `segments` of `pass`, atomically (a rolling pass replacing captions, a final pass
    /// replacing everything).
    pub async fn replace_segments(
        &self,
        meeting_id: i64,
        replaced: &[Pass],
        from_ms: i64,
        to_ms: i64,
        pass: Pass,
        segments: &[NewSegment],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for p in replaced {
            sqlx::query(
                "DELETE FROM segments
                 WHERE meeting_id = ? AND pass = ? AND start_ms >= ? AND start_ms < ?",
            )
            .bind(meeting_id)
            .bind(p.as_str())
            .bind(from_ms)
            .bind(to_ms)
            .execute(&mut *tx)
            .await?;
        }
        insert_segments_tx(&mut tx, meeting_id, pass, segments).await?;
        tx.commit().await?;
        Ok(())
    }

    /// All segments of a meeting in time order.
    pub async fn list_segments(&self, meeting_id: i64) -> Result<Vec<Segment>> {
        Ok(
            sqlx::query_as("SELECT * FROM segments WHERE meeting_id = ? ORDER BY start_ms, id")
                .bind(meeting_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn set_segment_speaker(
        &self,
        segment_id: i64,
        speaker_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query("UPDATE segments SET speaker_id = ? WHERE id = ?")
            .bind(speaker_id)
            .bind(segment_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------------------------------------------------------------- search

    /// Full-text search over all transcripts (prefix match per word, diacritics ignored).
    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let Some(fts) = fts_query(query) else {
            return Ok(Vec::new());
        };
        Ok(sqlx::query_as(
            "SELECT s.meeting_id, s.id AS segment_id, s.start_ms,
                    snippet(segments_fts, 0, char(1), char(2), '…', 12) AS snippet
             FROM segments_fts JOIN segments s ON s.id = segments_fts.rowid
             WHERE segments_fts MATCH ?
             ORDER BY rank LIMIT ?",
        )
        .bind(fts)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    // ---------------------------------------------------------------- speakers

    pub async fn create_speaker(&self, name: &str, color: i64) -> Result<i64> {
        let id = sqlx::query("INSERT INTO speakers (name, color) VALUES (?, ?)")
            .bind(name)
            .bind(color)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
        Ok(id)
    }

    pub async fn list_speakers(&self) -> Result<Vec<Speaker>> {
        Ok(
            sqlx::query_as("SELECT * FROM speakers ORDER BY name COLLATE NOCASE")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn rename_speaker(&self, id: i64, name: &str) -> Result<()> {
        sqlx::query("UPDATE speakers SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete a person and (by cascade) all their voice samples.
    pub async fn delete_speaker(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM speakers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Move `merged`'s voice samples and segments to `keep`, then delete `merged`.
    pub async fn merge_speakers(&self, keep: i64, merged: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for table in ["voice_samples", "segments", "meeting_voices"] {
            sqlx::query(&format!(
                "UPDATE {table} SET speaker_id = ? WHERE speaker_id = ?"
            ))
            .bind(keep)
            .bind(merged)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("UPDATE meeting_voices SET suggested_id = ? WHERE suggested_id = ?")
            .bind(keep)
            .bind(merged)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM speakers WHERE id = ?")
            .bind(merged)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn add_voice_sample(
        &self,
        speaker_id: i64,
        embedding: &[f32],
        encoder_id: &str,
        clean_seconds: f64,
        meeting_id: Option<i64>,
        created_at: i64,
    ) -> Result<i64> {
        let blob = to_blob(embedding);
        let id = sqlx::query(
            "INSERT INTO voice_samples
                 (speaker_id, embedding, encoder_id, clean_seconds, meeting_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(speaker_id)
        .bind(blob)
        .bind(encoder_id)
        .bind(clean_seconds)
        .bind(meeting_id)
        .bind(created_at)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// Voice samples made by one encoder (matching only compares like with like).
    pub async fn voice_samples(&self, encoder_id: &str) -> Result<Vec<VoiceSample>> {
        let rows: Vec<(i64, i64, Vec<u8>, f64)> = sqlx::query_as(
            "SELECT id, speaker_id, embedding, clean_seconds FROM voice_samples
             WHERE encoder_id = ? ORDER BY id",
        )
        .bind(encoder_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, speaker_id, blob, clean_seconds)| VoiceSample {
                id,
                speaker_id,
                embedding: from_blob(&blob),
                clean_seconds,
            })
            .collect())
    }

    // ---------------------------------------------------------------- settings

    /// A person by name, ignoring case.
    pub async fn speaker_by_name(&self, name: &str) -> Result<Option<Speaker>> {
        Ok(
            sqlx::query_as("SELECT * FROM speakers WHERE name = ? COLLATE NOCASE LIMIT 1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// People with the number of meetings they speak in and of voice samples.
    pub async fn list_people(&self) -> Result<Vec<Person>> {
        Ok(sqlx::query_as(
            "SELECT sp.id, sp.name, sp.color,
                    (SELECT COUNT(DISTINCT meeting_id) FROM segments WHERE speaker_id = sp.id) AS meetings,
                    (SELECT COUNT(*) FROM voice_samples WHERE speaker_id = sp.id) AS samples
             FROM speakers sp ORDER BY sp.name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ---------------------------------------------------------------- meeting voices

    /// Record a label's voice; keeps the embedding with the most clean speech.
    pub async fn upsert_meeting_voice(
        &self,
        meeting_id: i64,
        label: &str,
        embedding: Option<&[f32]>,
        encoder_id: &str,
        clean_seconds: f64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO meeting_voices (meeting_id, label, embedding, encoder_id, clean_seconds)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (meeting_id, label) DO UPDATE SET
                 embedding = excluded.embedding,
                 encoder_id = excluded.encoder_id,
                 clean_seconds = excluded.clean_seconds
             WHERE excluded.embedding IS NOT NULL
               AND (meeting_voices.embedding IS NULL
                    OR excluded.clean_seconds > meeting_voices.clean_seconds)",
        )
        .bind(meeting_id)
        .bind(label)
        .bind(embedding.map(to_blob))
        .bind(encoder_id)
        .bind(clean_seconds)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn meeting_voices(&self, meeting_id: i64) -> Result<Vec<MeetingVoice>> {
        type Row = (
            String,
            Option<i64>,
            Option<i64>,
            Option<f64>,
            Option<Vec<u8>>,
            Option<String>,
            f64,
            bool,
        );
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT label, speaker_id, suggested_id, distance, embedding, encoder_id,
                    clean_seconds, confirmed
             FROM meeting_voices WHERE meeting_id = ? ORDER BY CAST(label AS INTEGER), label",
        )
        .bind(meeting_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| MeetingVoice {
                label: r.0,
                speaker_id: r.1,
                suggested_id: r.2,
                distance: r.3,
                embedding: r.4.as_deref().map(from_blob),
                encoder_id: r.5,
                clean_seconds: r.6,
                confirmed: r.7,
            })
            .collect())
    }

    /// Who a label is: a match (auto-named or suggested) or the user's choice (`confirmed`).
    pub async fn set_voice_identity(
        &self,
        meeting_id: i64,
        label: &str,
        speaker_id: Option<i64>,
        suggested_id: Option<i64>,
        distance: Option<f64>,
        confirmed: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO meeting_voices (meeting_id, label, speaker_id, suggested_id, distance, confirmed)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (meeting_id, label) DO UPDATE SET
                 speaker_id = excluded.speaker_id, suggested_id = excluded.suggested_id,
                 distance = excluded.distance, confirmed = excluded.confirmed",
        )
        .bind(meeting_id)
        .bind(label)
        .bind(speaker_id)
        .bind(suggested_id)
        .bind(distance)
        .bind(confirmed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Forget a meeting's labels (the final pass numbers speakers anew).
    pub async fn clear_meeting_voices(&self, meeting_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM meeting_voices WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Give every segment of the meeting the person its label stands for (or none).
    pub async fn apply_voice_identities(&self, meeting_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE segments SET speaker_id = (
                 SELECT v.speaker_id FROM meeting_voices v
                 WHERE v.meeting_id = segments.meeting_id AND v.label = segments.speaker_label)
             WHERE meeting_id = ? AND speaker_label IS NOT NULL AND speaker_label <> 'me'",
        )
        .bind(meeting_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Two labels of a meeting are one person: `from`'s segments get label `into`; `into`
    /// takes `from`'s voice if it has more clean speech.
    pub async fn merge_labels(&self, meeting_id: i64, from: &str, into: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE segments SET speaker_label = ? WHERE meeting_id = ? AND speaker_label = ?",
        )
        .bind(into)
        .bind(meeting_id)
        .bind(from)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE meeting_voices SET
                 embedding = f.embedding, encoder_id = f.encoder_id, clean_seconds = f.clean_seconds
             FROM (SELECT embedding, encoder_id, clean_seconds FROM meeting_voices
                   WHERE meeting_id = ? AND label = ?) AS f
             WHERE meeting_voices.meeting_id = ? AND meeting_voices.label = ?
               AND f.embedding IS NOT NULL AND f.clean_seconds > meeting_voices.clean_seconds",
        )
        .bind(meeting_id)
        .bind(from)
        .bind(meeting_id)
        .bind(into)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM meeting_voices WHERE meeting_id = ? AND label = ?")
            .bind(meeting_id)
            .bind(from)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|(v,)| serde_json::from_str(&v).map_err(DbError::from))
            .transpose()
    }

    /// Point every stored path under `old` (a folder) at `new` instead: meeting folders, track
    /// files and the chosen meetings folder (the data folder moved, VRT-44). Returns how many.
    pub async fn rewrite_path_prefix(&self, old: &str, new: &str) -> Result<u64> {
        let (old, new) = (old.trim_end_matches('/'), new.trim_end_matches('/'));
        let mut n = 0;
        for (table, column) in [("meetings", "dir"), ("tracks", "path")] {
            let sql = format!(
                "UPDATE {table} SET {column} = ?1 || substr({column}, length(?2) + 1)
                 WHERE {column} = ?2 OR substr({column}, 1, length(?2) + 1) = ?2 || '/'"
            );
            n += sqlx::query(&sql)
                .bind(new)
                .bind(old)
                .execute(&self.pool)
                .await?
                .rows_affected();
        }
        let chosen: Option<Option<String>> =
            self.get_setting(crate::paths::MEETINGS_DIR_SETTING).await?;
        if let Some(Some(dir)) = chosen
            && (dir == old || dir.starts_with(&format!("{old}/")))
        {
            let moved = format!("{new}{}", &dir[old.len()..]);
            self.set_setting(crate::paths::MEETINGS_DIR_SETTING, &Some(moved))
                .await?;
            n += 1;
        }
        Ok(n)
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(serde_json::to_string(value)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn insert_segments_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    meeting_id: i64,
    pass: Pass,
    segments: &[NewSegment],
) -> Result<()> {
    for s in segments {
        sqlx::query(
            "INSERT INTO segments (meeting_id, track_id, pass, speaker_id, speaker_label,
                                   start_ms, end_ms, text, words_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(meeting_id)
        .bind(s.track_id)
        .bind(pass.as_str())
        .bind(s.speaker_id)
        .bind(&s.speaker_label)
        .bind(s.start_ms)
        .bind(s.end_ms)
        .bind(&s.text)
        .bind(&s.words_json)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Turn user input into a safe FTS5 query: every word quoted, prefix-matched, AND-ed.
fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> Db {
        Db::open_in_memory().await.unwrap()
    }

    fn seg(start_ms: i64, text: &str) -> NewSegment {
        NewSegment {
            start_ms,
            end_ms: start_ms + 1000,
            text: text.into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn sync_rows_round_trip_and_go_with_the_meeting() {
        let db = db().await;
        let m = db.create_meeting("Planning", 1_000, "/m/p").await.unwrap();
        db.add_sync_meeting(m, "Manent/2026-11-02 14.05 Planning", "phone")
            .await
            .unwrap();
        db.set_sync_uploaded(m, Some("abc"), true).await.unwrap();
        let row = db.sync_meeting(m).await.unwrap().unwrap();
        assert_eq!(
            (
                row.origin.as_str(),
                row.transcript_hash.as_deref(),
                row.mix_uploaded,
                row.gone
            ),
            ("phone", Some("abc"), true, false)
        );
        db.set_inbox_item("Manent/Inbox/a.opus", "e1", None, "importing", None)
            .await
            .unwrap();
        db.set_inbox_item("Manent/Inbox/a.opus", "e1", Some(m), "done", None)
            .await
            .unwrap();
        let items = db.inbox_items().await.unwrap();
        assert_eq!(
            (items.len(), items[0].meeting_id, items[0].state.as_str()),
            (1, Some(m), "done")
        );
        db.delete_meeting(m).await.unwrap();
        assert!(db.sync_meeting(m).await.unwrap().is_none(), "cascade");
        assert_eq!(
            db.inbox_items().await.unwrap()[0].meeting_id,
            None,
            "set null: still known"
        );
        db.clear_sync().await.unwrap();
        assert!(db.inbox_items().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn gaps_are_stored_in_order_and_go_with_the_meeting() {
        let db = db().await;
        let m = db.create_meeting("Long", 1_000, "/m/long").await.unwrap();
        db.add_gaps(m, &[(900_000, 60_000), (120_000, 420_000)])
            .await
            .unwrap();
        assert_eq!(
            db.list_gaps(m).await.unwrap(),
            vec![(120_000, 420_000), (900_000, 60_000)]
        );
        db.delete_meeting(m).await.unwrap();
        assert!(db.list_gaps(m).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn schema_has_the_doc_2_tables() {
        let db = db().await;
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type IN ('table') ORDER BY name")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        let names: Vec<_> = tables.into_iter().map(|(n,)| n).collect();
        for t in [
            "meetings",
            "tracks",
            "segments",
            "speakers",
            "voice_samples",
            "meeting_voices",
            "meeting_gaps",
            "settings",
            "segments_fts",
        ] {
            assert!(
                names.iter().any(|n| n == t),
                "missing table {t} in {names:?}"
            );
        }
    }

    #[tokio::test]
    async fn meetings_create_list_rename_finish_delete() {
        let db = db().await;
        let a = db.create_meeting("Standup", 1_000, "/m/a").await.unwrap();
        let b = db.create_meeting("Review", 2_000, "/m/b").await.unwrap();
        let list = db.list_meetings().await.unwrap();
        assert_eq!(
            list.iter().map(|m| m.id).collect::<Vec<_>>(),
            [b, a],
            "newest first"
        );
        assert_eq!(list[1].status, "recording");
        db.rename_meeting(a, "Daily standup").await.unwrap();
        db.set_meeting_dir(a, "/m/1").await.unwrap();
        db.finish_meeting(a, 90_000, "done").await.unwrap();
        let m = db.get_meeting(a).await.unwrap().unwrap();
        assert_eq!(
            (m.title.as_str(), m.duration_ms, m.status.as_str()),
            ("Daily standup", 90_000, "done")
        );
        assert!(db.delete_meeting(a).await.unwrap());
        assert!(!db.delete_meeting(a).await.unwrap());
        assert!(db.get_meeting(a).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn meeting_speakers_are_distinct_and_per_meeting() {
        let db = db().await;
        let m1 = db.create_meeting("a", 1, "/a").await.unwrap();
        let m2 = db.create_meeting("b", 2, "/b").await.unwrap();
        let anna = db.create_speaker("anna", 3).await.unwrap();
        let bram = db.create_speaker("Bram", 1).await.unwrap();
        let named = |id, start| NewSegment {
            speaker_id: Some(id),
            ..seg(start, "x")
        };
        db.insert_segments(
            m1,
            Pass::Final,
            &[
                named(bram, 0),
                named(anna, 1),
                named(bram, 2),
                seg(3, "unnamed"),
            ],
        )
        .await
        .unwrap();
        db.insert_segments(m2, Pass::Final, &[named(anna, 0)])
            .await
            .unwrap();
        let got: Vec<_> = db
            .meeting_speakers()
            .await
            .unwrap()
            .into_iter()
            .map(|(m, s)| (m, s.name))
            .collect();
        assert_eq!(
            got,
            [
                (m1, "anna".into()),
                (m1, "Bram".into()),
                (m2, "anna".into())
            ]
        );
    }

    #[tokio::test]
    async fn invalid_status_is_rejected() {
        let db = db().await;
        let a = db.create_meeting("x", 1, "/m").await.unwrap();
        assert!(db.finish_meeting(a, 1, "bogus").await.is_err());
    }

    #[tokio::test]
    async fn deleting_a_meeting_cascades_to_tracks_and_segments() {
        let db = db().await;
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        let track = NewTrack {
            source_kind: "mic",
            source_id: "pipewire:x",
            device_name: "Mic",
            path: "/m/mic.flac",
            seconds: 3.5,
        };
        let t = db.add_track(m, &track).await.unwrap();
        assert_eq!(db.list_tracks(m).await.unwrap()[0].id, t);
        db.set_track_seconds(t, 7.25).await.unwrap();
        assert_eq!(db.list_tracks(m).await.unwrap()[0].seconds, 7.25);
        db.insert_segments(m, Pass::Final, &[seg(0, "hallo wereld")])
            .await
            .unwrap();
        db.delete_meeting(m).await.unwrap();
        assert!(db.list_tracks(m).await.unwrap().is_empty());
        assert!(db.list_segments(m).await.unwrap().is_empty());
        assert!(
            db.search("hallo", 10).await.unwrap().is_empty(),
            "fts row removed too"
        );
    }

    #[tokio::test]
    async fn meeting_voices_keep_the_cleanest_voice_and_name_segments() {
        let db = db().await;
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        let anna = db.create_speaker("Anna", 2).await.unwrap();
        let labelled = |label: &str, start_ms| NewSegment {
            speaker_label: Some(label.into()),
            ..seg(start_ms, "x")
        };
        db.insert_segments(
            m,
            Pass::Rolling,
            &[labelled("1", 0), labelled("2", 1000), labelled("me", 2000)],
        )
        .await
        .unwrap();
        db.upsert_meeting_voice(m, "1", Some(&[1.0, 0.0]), "sha256:e", 8.0)
            .await
            .unwrap();
        db.upsert_meeting_voice(m, "1", Some(&[0.0, 1.0]), "sha256:e", 3.0)
            .await
            .unwrap();
        db.upsert_meeting_voice(m, "1", None, "sha256:e", 30.0)
            .await
            .unwrap();
        db.upsert_meeting_voice(m, "2", Some(&[0.6, 0.8]), "sha256:e", 12.0)
            .await
            .unwrap();
        let v = db.meeting_voices(m).await.unwrap();
        assert_eq!(
            v[0].embedding.as_deref(),
            Some(&[1.0f32, 0.0][..]),
            "cleanest kept"
        );
        assert_eq!(v[0].clean_seconds, 8.0);

        db.set_voice_identity(m, "1", Some(anna), None, Some(0.1), true)
            .await
            .unwrap();
        db.apply_voice_identities(m).await.unwrap();
        let speakers: Vec<_> = db
            .list_segments(m)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.speaker_id)
            .collect();
        assert_eq!(speakers, [Some(anna), None, None]);
        let v = db.meeting_voices(m).await.unwrap();
        assert!(
            v[0].confirmed && v[0].embedding.is_some(),
            "identity keeps the voice"
        );

        // Speaker 2 turns out to be Anna too.
        db.merge_labels(m, "2", "1").await.unwrap();
        db.apply_voice_identities(m).await.unwrap();
        let segs = db.list_segments(m).await.unwrap();
        assert_eq!(segs[1].speaker_label.as_deref(), Some("1"));
        assert_eq!(segs[1].speaker_id, Some(anna));
        let v = db.meeting_voices(m).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].clean_seconds, 12.0,
            "the cleaner voice of the merged label"
        );

        db.delete_speaker(anna).await.unwrap();
        assert_eq!(db.meeting_voices(m).await.unwrap()[0].speaker_id, None);
        db.clear_meeting_voices(m).await.unwrap();
        assert!(db.meeting_voices(m).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rolling_pass_replaces_captions_in_its_window_only() {
        let db = db().await;
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        db.insert_segments(
            m,
            Pass::Caption,
            &[seg(0, "a"), seg(30_000, "b"), seg(70_000, "c")],
        )
        .await
        .unwrap();
        db.replace_segments(
            m,
            &[Pass::Caption],
            0,
            60_000,
            Pass::Rolling,
            &[seg(0, "A"), seg(31_000, "B")],
        )
        .await
        .unwrap();
        let s = db.list_segments(m).await.unwrap();
        let got: Vec<_> = s
            .iter()
            .map(|x| (x.text.as_str(), x.pass.as_str()))
            .collect();
        assert_eq!(got, [("A", "rolling"), ("B", "rolling"), ("c", "caption")]);
    }

    #[tokio::test]
    async fn search_finds_dutch_and_english_ignoring_diacritics_and_case() {
        let db = db().await;
        let m1 = db.create_meeting("m1", 1, "/m1").await.unwrap();
        let m2 = db.create_meeting("m2", 2, "/m2").await.unwrap();
        db.insert_segments(
            m1,
            Pass::Final,
            &[
                seg(0, "We hebben één ideeën-lijst"),
                seg(5_000, "De begroting is rond"),
            ],
        )
        .await
        .unwrap();
        db.insert_segments(
            m2,
            Pass::Final,
            &[seg(1_000, "The Budget meeting went well")],
        )
        .await
        .unwrap();

        let hits = db.search("ideeen", 10).await.unwrap();
        assert_eq!(hits.len(), 1, "diacritics ignored");
        assert_eq!(hits[0].meeting_id, m1);
        assert!(
            hits[0].snippet.contains("\u{1}ideeën\u{2}"),
            "{}",
            hits[0].snippet
        );
        assert_eq!(
            db.search("BUDG", 10).await.unwrap()[0].meeting_id,
            m2,
            "prefix, case-insensitive"
        );
        assert_eq!(
            db.search("begroting rond", 10).await.unwrap().len(),
            1,
            "all words"
        );
        assert!(db.search("begroting budget", 10).await.unwrap().is_empty());
        assert!(db.search("  ", 10).await.unwrap().is_empty());
        assert!(
            db.search("\"oops", 10).await.is_ok(),
            "quotes can't break the query"
        );
    }

    #[tokio::test]
    async fn search_follows_updates() {
        let db = db().await;
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        db.insert_segments(m, Pass::Caption, &[seg(0, "oude tekst")])
            .await
            .unwrap();
        db.replace_segments(
            m,
            &[Pass::Caption],
            0,
            10_000,
            Pass::Final,
            &[seg(0, "nieuwe tekst")],
        )
        .await
        .unwrap();
        assert!(db.search("oude", 10).await.unwrap().is_empty());
        assert_eq!(db.search("nieuwe", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn speakers_samples_merge_and_delete() {
        let db = db().await;
        let m = db.create_meeting("m", 1, "/m").await.unwrap();
        let bram = db.create_speaker("Bram", 1).await.unwrap();
        let dup = db.create_speaker("bram (2)", 2).await.unwrap();
        db.add_voice_sample(bram, &[0.5, -0.25], "sha256:enc", 30.0, Some(m), 10)
            .await
            .unwrap();
        db.add_voice_sample(dup, &[0.1, 0.2], "sha256:enc", 12.0, Some(m), 11)
            .await
            .unwrap();
        db.add_voice_sample(dup, &[9.0], "sha256:other", 5.0, None, 12)
            .await
            .unwrap();
        db.insert_segments(
            m,
            Pass::Final,
            &[NewSegment {
                speaker_id: Some(dup),
                ..seg(0, "hoi")
            }],
        )
        .await
        .unwrap();

        let samples = db.voice_samples("sha256:enc").await.unwrap();
        assert_eq!(samples.len(), 2, "only the requested encoder");
        assert_eq!(samples[0].embedding, [0.5, -0.25], "embedding round-trips");

        db.merge_speakers(bram, dup).await.unwrap();
        assert_eq!(db.list_speakers().await.unwrap().len(), 1);
        assert!(
            db.voice_samples("sha256:enc")
                .await
                .unwrap()
                .iter()
                .all(|s| s.speaker_id == bram)
        );
        assert_eq!(db.list_segments(m).await.unwrap()[0].speaker_id, Some(bram));

        assert_eq!(
            db.speaker_by_name("BRAM").await.unwrap().map(|s| s.id),
            Some(bram)
        );
        let people = db.list_people().await.unwrap();
        assert_eq!(
            (people[0].meetings, people[0].samples),
            (1, 3),
            "samples of every encoder"
        );
        db.rename_speaker(bram, "Bram Jansen").await.unwrap();
        assert_eq!(db.list_speakers().await.unwrap()[0].name, "Bram Jansen");
        db.delete_speaker(bram).await.unwrap();
        assert!(
            db.voice_samples("sha256:enc").await.unwrap().is_empty(),
            "voice data deleted"
        );
        assert!(db.voice_samples("sha256:other").await.unwrap().is_empty());
        assert_eq!(
            db.list_segments(m).await.unwrap()[0].speaker_id,
            None,
            "segments kept, unnamed"
        );
    }

    #[tokio::test]
    async fn settings_round_trip_json() {
        let db = db().await;
        assert_eq!(db.get_setting::<String>("mic").await.unwrap(), None);
        db.set_setting("mic", &"pipewire:input_default")
            .await
            .unwrap();
        db.set_setting("mic", &"pipewire:other").await.unwrap();
        assert_eq!(
            db.get_setting::<String>("mic").await.unwrap().as_deref(),
            Some("pipewire:other")
        );
        db.set_setting("latency", &vec![1, 2]).await.unwrap();
        assert_eq!(
            db.get_setting::<Vec<i32>>("latency").await.unwrap(),
            Some(vec![1, 2])
        );
    }

    #[test]
    fn fts_query_quotes_and_prefixes_terms() {
        assert_eq!(
            fts_query("hallo wereld").as_deref(),
            Some("\"hallo\"* \"wereld\"*")
        );
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"ab\"*"));
        assert_eq!(fts_query("   "), None);
    }

    #[tokio::test]
    async fn file_database_persists_and_migrates_once() {
        let dir = crate::test_util::temp_dir();
        let path = dir.join("manent.db");
        {
            let db = Db::open(&path).await.unwrap();
            db.create_meeting("kept", 1, "/m").await.unwrap();
        }
        let db = Db::open(&path).await.unwrap();
        assert_eq!(db.list_meetings().await.unwrap()[0].title, "kept");
        let migrations: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(migrations.0, 6, "each migration applied once");
    }
}
