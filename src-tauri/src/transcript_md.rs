//! `transcript.md` next to a meeting's audio (VRT-37, decision-5): the transcript as Markdown
//! with a YAML header, readable without the app (in Nextcloud, any editor, the companion app)
//! and parsed back by [`parse`].
//!
//! ```text
//! ---
//! manent: 1
//! id: 42
//! title: "Kwartaaloverleg"
//! started_at: "2026-10-31T13:05:00Z"
//! duration_ms: 3725000
//! audio: ["mix.opus", "mic.flac", "system.flac"]
//! pass: "final"
//! speakers: ["Anna", "Rob", "Speaker 3"]
//! gaps: [{"at_ms": 120000, "missing_ms": 420000}]
//! ---
//!
//! # Kwartaaloverleg
//!
//! **Anna** · 0:01
//!
//! Goedemorgen allemaal.
//!
//! > No audio for 7 min at 2:00: the computer was asleep
//! ```
//!
//! Every header value is written as JSON, which is also valid YAML, so the header needs no
//! YAML library either way. Speaker names and labels only: never voice embeddings. Times in
//! the body have whole seconds (the phone seeks per block); word timings are not included.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::db::{Db, Meeting};
use crate::export::{self, Block, Export};

pub const FILE: &str = "transcript.md";
/// Bumped when the format changes in a way readers must know about.
pub const SCHEMA: u64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub id: i64,
    pub title: String,
    /// ISO 8601, UTC.
    pub started_at: String,
    pub duration_ms: i64,
    /// Audio files in the meeting folder this transcript belongs to.
    pub audio: Vec<String>,
    /// "final", or the pass of the newest segments while a meeting is still being transcribed.
    pub pass: String,
    pub speakers: Vec<String>,
    /// (at_ms, missing_ms): audio lost while the computer slept (VRT-36).
    pub gaps: Vec<(i64, i64)>,
}

// The reader is for the companion app and for importing transcripts later (decision-5);
// in the desktop app only the tests use it so far.

#[cfg_attr(not(test), allow(dead_code))]
/// A block as read back: who, when (whole seconds) and what.
#[derive(Debug, Clone, PartialEq)]
pub struct DocBlock {
    pub speaker: String,
    pub start_ms: u32,
    pub text: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub schema: u64,
    pub header: Header,
    pub blocks: Vec<DocBlock>,
}

/// `transcript.md` for a meeting.
pub fn render(header: &Header, blocks: &[Block]) -> String {
    let s = |v: &str| Value::from(v).to_string();
    let list = |v: &[String]| Value::from(v.to_vec()).to_string();
    let gaps: Vec<Value> = header
        .gaps
        .iter()
        .map(|(at, missing)| json!({"at_ms": at, "missing_ms": missing}))
        .collect();
    let mut out = format!(
        "---\nmanent: {SCHEMA}\nid: {}\ntitle: {}\nstarted_at: {}\nduration_ms: {}\naudio: {}\npass: {}\nspeakers: {}\ngaps: {}\n---\n\n# {}\n",
        header.id,
        s(&header.title),
        s(&header.started_at),
        header.duration_ms,
        list(&header.audio),
        s(&header.pass),
        list(&header.speakers),
        Value::from(gaps),
        heading(&header.title),
    );
    let mut gaps = header.gaps.iter().peekable();
    for b in blocks {
        while let Some((at, missing)) = gaps.next_if(|(at, _)| *at <= i64::from(b.start_ms)) {
            out.push_str(&gap_line(*at, *missing));
        }
        out.push_str(&format!(
            "\n**{}** · {}\n\n{}\n",
            b.speaker,
            export::clock(b.start_ms),
            one_line(&b.text)
        ));
    }
    for (at, missing) in gaps {
        out.push_str(&gap_line(*at, *missing));
    }
    out
}

fn heading(title: &str) -> &str {
    if title.trim().is_empty() {
        "Untitled meeting"
    } else {
        title.trim()
    }
}

/// Block text on one line, so a paragraph is always exactly one block's text.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn gap_line(at_ms: i64, missing_ms: i64) -> String {
    let s = (missing_ms.max(0) + 500) / 1000;
    let length = if s < 60 {
        format!("{s} s")
    } else {
        let m = (s + 30) / 60;
        if m < 60 {
            format!("{m} min")
        } else {
            format!("{} h {} min", m / 60, m % 60)
        }
    };
    format!(
        "\n> No audio for {length} at {}: the computer was asleep\n",
        export::clock(at_ms.max(0) as u32)
    )
}

#[cfg_attr(not(test), allow(dead_code))]
/// Read a `transcript.md` back. Strict about the header (it is machine-written); the body is
/// read as paragraphs: `**Speaker** · m:ss` starts a block, the next paragraph is its text,
/// `#` headings and `>` notes are skipped.
pub fn parse(text: &str) -> Result<Doc, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("no YAML header (must start with ---)")?;
    let (head, body) = rest.split_once("\n---\n").ok_or("YAML header not closed")?;
    let mut fields: HashMap<&str, Value> = HashMap::new();
    for line in head.lines().filter(|l| !l.trim().is_empty()) {
        let (key, value) = line
            .split_once(": ")
            .ok_or_else(|| format!("bad header line: {line}"))?;
        let value: Value =
            serde_json::from_str(value).map_err(|e| format!("bad value for {key}: {e}"))?;
        fields.insert(key, value);
    }
    let get = |key: &str| fields.get(key).ok_or_else(|| format!("missing {key}"));
    let string = |key: &str| -> Result<String, String> {
        get(key)?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{key} is not a string"))
    };
    let int = |key: &str| {
        get(key)?
            .as_i64()
            .ok_or_else(|| format!("{key} is not a number"))
    };
    let strings = |key: &str| -> Result<Vec<String>, String> {
        get(key)?
            .as_array()
            .ok_or_else(|| format!("{key} is not a list"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or(format!("{key}: not a string"))
            })
            .collect()
    };
    // Files written before the rename (VRT-44) say "veritas: 1".
    let schema = int("manent").or_else(|_| int("veritas"))? as u64;
    if schema > SCHEMA {
        return Err(format!("written by a newer Manent (format {schema})"));
    }
    let gaps = get("gaps")?
        .as_array()
        .ok_or("gaps is not a list")?
        .iter()
        .map(|g| match (g["at_ms"].as_i64(), g["missing_ms"].as_i64()) {
            (Some(a), Some(m)) => Ok((a, m)),
            _ => Err("bad gap".to_string()),
        })
        .collect::<Result<_, _>>()?;
    let header = Header {
        id: int("id")?,
        title: string("title")?,
        started_at: string("started_at")?,
        duration_ms: int("duration_ms")?,
        audio: strings("audio")?,
        pass: string("pass")?,
        speakers: strings("speakers")?,
        gaps,
    };
    let mut blocks: Vec<DocBlock> = Vec::new();
    let mut open = false; // a block header waits for its text
    for para in body.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if let Some((speaker, at)) = block_header(para) {
            blocks.push(DocBlock {
                speaker,
                start_ms: at,
                text: String::new(),
            });
            open = true;
        } else if para.starts_with("# ") || para.starts_with("> ") {
            open = false;
        } else if open && let Some(b) = blocks.last_mut() {
            b.text = para.to_string();
            open = false;
        }
    }
    Ok(Doc {
        schema,
        header,
        blocks,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
/// `**Name** · 1:02:03` -> ("Name", 3723000).
fn block_header(line: &str) -> Option<(String, u32)> {
    let rest = line.strip_prefix("**")?;
    let (speaker, time) = rest.rsplit_once("** · ")?;
    if speaker.is_empty() || line.contains('\n') {
        return None;
    }
    let mut secs = 0u32;
    let parts: Vec<&str> = time.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    for p in parts {
        secs = secs * 60 + p.parse::<u32>().ok()?;
    }
    Some((speaker.to_string(), secs * 1000))
}

/// The newest pass among the segments: "final" once the final pass ran.
fn pass_of(passes: &[String]) -> String {
    ["final", "rolling", "caption"]
        .iter()
        .find(|p| passes.iter().any(|x| x == *p))
        .unwrap_or(&"final")
        .to_string()
}

/// Everything an export needs for a meeting, with speakers named as the app shows them.
/// None if there is no such meeting.
pub async fn load(
    db: &Db,
    id: i64,
    date_label: String,
) -> Result<Option<(Meeting, Export, Vec<String>)>, String> {
    let err = |e: crate::db::DbError| e.to_string();
    let Some(meeting) = db.get_meeting(id).await.map_err(err)? else {
        return Ok(None);
    };
    let tracks = db.list_tracks(id).await.map_err(err)?;
    let segments = db.list_segments(id).await.map_err(err)?;
    let people: HashMap<i64, String> = db
        .list_speakers()
        .await
        .map_err(err)?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let me: Option<String> = db
        .get_setting(crate::commands::ME_NAME_SETTING)
        .await
        .map_err(err)?;
    let me = me
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Me".into());
    let passes: Vec<String> = segments.iter().map(|s| s.pass.clone()).collect();
    let export = Export {
        title: meeting.title.clone(),
        date_label,
        started_at: meeting.started_at,
        duration_ms: meeting.duration_ms,
        parts: export::parts(&segments, &tracks, &people, &me),
    };
    Ok(Some((meeting, export, passes)))
}

/// Write (or rewrite) a meeting's `transcript.md`, atomically. Nothing is written while the
/// meeting has no transcript; an existing file is then removed (e.g. all text was deleted).
/// Returns the path written.
pub async fn write(db: &Db, id: i64) -> Result<Option<PathBuf>, String> {
    let Some((meeting, export, passes)) = load(db, id, String::new()).await? else {
        return Ok(None);
    };
    let dir = PathBuf::from(&meeting.dir);
    let path = dir.join(FILE);
    let blocks = export::blocks(&export.parts);
    if blocks.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    }
    if !dir.is_dir() {
        return Err(format!("meeting folder {} is missing", dir.display()));
    }
    let mut speakers: Vec<String> = Vec::new();
    for b in &blocks {
        if !speakers.contains(&b.speaker) {
            speakers.push(b.speaker.clone());
        }
    }
    let header = Header {
        id: meeting.id,
        title: meeting.title.clone(),
        started_at: export::iso_utc(meeting.started_at),
        duration_ms: meeting.duration_ms,
        audio: audio_files(db, &meeting).await?,
        pass: pass_of(&passes),
        speakers,
        gaps: db.list_gaps(id).await.map_err(|e| e.to_string())?,
    };
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, render(&header, &blocks))
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| format!("can't write {}: {e}", path.display()))?;
    // A changed transcript goes to Nextcloud at once when sync is on (VRT-39).
    crate::sync::wake();
    Ok(Some(path))
}

/// The playback file and the tracks that are still there, as names in the meeting folder.
async fn audio_files(db: &Db, meeting: &Meeting) -> Result<Vec<String>, String> {
    let dir = Path::new(&meeting.dir);
    let mut names = Vec::new();
    if dir.join(crate::media_server::MIX_FILE).is_file() {
        names.push(crate::media_server::MIX_FILE.to_string());
    }
    for t in db
        .list_tracks(meeting.id)
        .await
        .map_err(|e| e.to_string())?
    {
        let path = Path::new(&t.path);
        if path.is_file()
            && let Some(name) = path.file_name()
        {
            names.push(name.to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

/// Rewrite these meetings' files in the background; failures are logged, never shown.
pub fn spawn_write(db: Db, ids: Vec<i64>) {
    tauri::async_runtime::spawn(async move {
        for id in ids {
            if let Err(e) = write(&db, id).await {
                eprintln!("transcript.md for meeting {id}: {e}");
            }
        }
    });
}

/// Rewrite every meeting's file (a change that touches many: a person renamed, merged or
/// deleted, the user's own name).
pub fn spawn_write_all(db: Db) {
    tauri::async_runtime::spawn(async move {
        match db.list_meetings().await {
            Ok(list) => spawn_write(db, list.into_iter().map(|m| m.id).collect()),
            Err(e) => eprintln!("transcript.md: listing meetings failed: {e}"),
        }
    });
}

/// At startup: meetings without a `transcript.md` get one (meetings from before VRT-37).
pub fn spawn_backfill(db: Db) {
    tauri::async_runtime::spawn(async move {
        let Ok(list) = db.list_meetings().await else {
            return;
        };
        let missing: Vec<i64> = list
            .into_iter()
            .filter(|m| {
                let file = Path::new(&m.dir).join(FILE);
                // Missing, or written before the rename to Manent (VRT-44).
                m.status == "done"
                    && std::fs::read_to_string(&file)
                        .map_or(true, |t| t.starts_with("---\nveritas:"))
            })
            .map(|m| m.id)
            .collect();
        if !missing.is_empty() {
            eprintln!("transcript.md: writing {} missing file(s)", missing.len());
        }
        spawn_write(db, missing);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::transcript::Word;

    fn block(speaker: &str, start_ms: u32, text: &str) -> Block {
        Block {
            speaker: speaker.into(),
            start_ms,
            end_ms: start_ms + 2000,
            text: text.into(),
            words: vec![Word {
                text: "x".into(),
                start_ms,
                end_ms: start_ms + 100,
                conf: 0.9,
            }],
        }
    }

    fn header() -> Header {
        Header {
            id: 42,
            title: "Kwartaaloverleg \"Q4\": budget".into(),
            started_at: "2026-10-31T13:05:00Z".into(),
            duration_ms: 3_725_000,
            audio: vec!["mix.opus".into(), "mic.flac".into(), "system.flac".into()],
            pass: "final".into(),
            speakers: vec!["Anna".into(), "Rob".into(), "Speaker 3".into()],
            gaps: vec![(120_000, 420_000)],
        }
    }

    fn blocks() -> Vec<Block> {
        vec![
            block("Anna", 1_200, "Goedemorgen allemaal. Shall we start?"),
            block("Rob", 4_000, "Yes,\nlet's go — **now**."),
            block("Speaker 3", 125_000, "Ja, en dat is precies waarom."),
            block("Anna", 3_600_000, "Eén vraag nog: # niet een kop."),
        ]
    }

    #[test]
    fn snapshot() {
        let text = render(&header(), &blocks());
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/transcript.md");
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        let want = std::fs::read_to_string(&path).expect("snapshot (UPDATE_SNAPSHOTS=1 to create)");
        assert_eq!(text, want);
    }

    #[test]
    fn round_trip_keeps_header_and_blocks() {
        let doc = parse(&render(&header(), &blocks())).unwrap();
        assert_eq!(doc.schema, SCHEMA);
        assert_eq!(doc.header, header());
        let got: Vec<(&str, u32, &str)> = doc
            .blocks
            .iter()
            .map(|b| (b.speaker.as_str(), b.start_ms, b.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("Anna", 1_000, "Goedemorgen allemaal. Shall we start?"),
                ("Rob", 4_000, "Yes, let's go — **now**."),
                ("Speaker 3", 125_000, "Ja, en dat is precies waarom."),
                ("Anna", 3_600_000, "Eén vraag nog: # niet een kop."),
            ]
        );
    }

    #[test]
    fn the_gap_note_sits_where_the_audio_stopped() {
        let text = render(&header(), &blocks());
        let note = text.find("> No audio for 7 min at 2:00").unwrap();
        assert!(text.find("**Rob** · 0:04").unwrap() < note);
        assert!(note < text.find("**Speaker 3** · 2:05").unwrap());
    }

    #[test]
    fn files_from_before_the_rename_still_parse() {
        let old = render(&header(), &blocks()).replace("manent: 1", "veritas: 1");
        let doc = parse(&old).unwrap();
        assert_eq!((doc.schema, doc.header.id), (1, 42));
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        assert!(parse("# just markdown\n").is_err());
        assert!(parse("---\nmanent: 1\n").is_err(), "unclosed header");
        let newer = render(&header(), &blocks()).replace("manent: 1", "manent: 99");
        assert!(parse(&newer).unwrap_err().contains("newer"));
        let broken =
            render(&header(), &blocks()).replace("duration_ms: 3725000", "duration_ms: long");
        assert!(parse(&broken).is_err());
    }

    #[test]
    fn newest_pass_is_reported() {
        let p = |v: &[&str]| pass_of(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(p(&["final", "final"]), "final");
        assert_eq!(p(&["caption", "rolling"]), "rolling");
        assert_eq!(p(&["caption"]), "caption");
    }
}
