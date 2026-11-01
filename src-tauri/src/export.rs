//! Transcript export (VRT-21): Markdown, plain text, SRT subtitles and JSON, with speakers as
//! the app shows them and times on the playback timeline.

use serde::Serialize;
use serde_json::json;

use std::collections::HashMap;

use crate::db::{Segment, Track};
use crate::engine::transcript::Word;

/// A new block starts when the speaker changes or after a pause this long (as in the app).
const BLOCK_GAP_MS: u32 = 1500;
/// SRT cues are at most this long; longer blocks are split at word boundaries.
const MAX_CUE_MS: u32 = 7000;

/// One transcript segment as stored, with its speaker already named.
#[derive(Debug, Clone)]
pub struct Part {
    pub speaker: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

/// What gets exported.
pub struct Export {
    pub title: String,
    /// Shown as given (the UI formats it in the user's locale).
    pub date_label: String,
    /// Unix ms, for JSON.
    pub started_at: i64,
    pub duration_ms: i64,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Block {
    pub speaker: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Text,
    Srt,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "md" => Self::Markdown,
            "txt" => Self::Text,
            "srt" => Self::Srt,
            "json" => Self::Json,
            _ => return None,
        })
    }
}

/// Stored segments with the speaker as the app shows it: the person's name, the user's own
/// name for the mic, "Speaker N", or for a live caption its source.
pub fn parts(
    segments: &[Segment],
    tracks: &[Track],
    people: &HashMap<i64, String>,
    me_name: &str,
) -> Vec<Part> {
    segments
        .iter()
        .map(|s| {
            let mic = tracks
                .iter()
                .find(|t| Some(t.id) == s.track_id)
                .is_some_and(|t| t.source_kind == "mic");
            let speaker = match (
                s.speaker_id.and_then(|id| people.get(&id)),
                s.speaker_label.as_deref(),
            ) {
                (Some(name), _) => name.clone(),
                (None, Some("me")) => me_name.to_string(),
                (None, Some(label)) => format!("Speaker {label}"),
                (None, None) if mic => me_name.to_string(),
                (None, None) => "System audio".to_string(),
            };
            Part {
                speaker,
                start_ms: s.start_ms.max(0) as u32,
                end_ms: s.end_ms.max(0) as u32,
                text: s.text.clone(),
                words: s
                    .words_json
                    .as_deref()
                    .and_then(|w| serde_json::from_str(w).ok())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// Parts in time order merged into speaker blocks.
pub fn blocks(parts: &[Part]) -> Vec<Block> {
    let mut sorted: Vec<&Part> = parts.iter().filter(|p| !p.text.trim().is_empty()).collect();
    sorted.sort_by_key(|p| (p.start_ms, p.end_ms));
    let mut out: Vec<Block> = Vec::new();
    for p in sorted {
        match out.last_mut() {
            Some(b)
                if b.speaker == p.speaker && p.start_ms.saturating_sub(b.end_ms) < BLOCK_GAP_MS =>
            {
                b.end_ms = b.end_ms.max(p.end_ms);
                b.text.push(' ');
                b.text.push_str(p.text.trim());
                b.words.extend(p.words.iter().cloned());
            }
            _ => out.push(Block {
                speaker: p.speaker.clone(),
                start_ms: p.start_ms,
                end_ms: p.end_ms,
                text: p.text.trim().to_string(),
                words: p.words.clone(),
            }),
        }
    }
    out
}

/// `h:mm:ss` (or `m:ss` under an hour).
pub(crate) fn clock(ms: u32) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// `hh:mm:ss,mmm` for SRT.
fn srt_time(ms: u32) -> String {
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02},{f:03}")
}

/// ISO 8601 UTC for a Unix ms time (no time zone library needed for UTC).
pub(crate) fn iso_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn title(e: &Export) -> &str {
    if e.title.trim().is_empty() {
        "Untitled meeting"
    } else {
        e.title.trim()
    }
}

pub fn render(e: &Export, format: Format) -> String {
    let blocks = blocks(&e.parts);
    match format {
        Format::Markdown => {
            let mut out = format!(
                "# {}\n\n{} · {}\n",
                title(e),
                e.date_label,
                clock(e.duration_ms.max(0) as u32)
            );
            for b in &blocks {
                out.push_str(&format!(
                    "\n**{}** · {}\n\n{}\n",
                    b.speaker,
                    clock(b.start_ms),
                    b.text
                ));
            }
            out
        }
        Format::Text => {
            let mut out = format!(
                "{}\n{} · {}\n",
                title(e),
                e.date_label,
                clock(e.duration_ms.max(0) as u32)
            );
            for b in &blocks {
                out.push_str(&format!(
                    "\n[{}] {}: {}\n",
                    clock(b.start_ms),
                    b.speaker,
                    b.text
                ));
            }
            out
        }
        Format::Srt => {
            let mut out = String::new();
            let mut n = 0;
            for b in &blocks {
                for (start, end, text) in cues(b) {
                    n += 1;
                    out.push_str(&format!(
                        "{n}\n{} --> {}\n{}: {}\n\n",
                        srt_time(start),
                        srt_time(end),
                        b.speaker,
                        text
                    ));
                }
            }
            out
        }
        Format::Json => {
            let mut speakers: Vec<&str> = Vec::new();
            for b in &blocks {
                if !speakers.contains(&b.speaker.as_str()) {
                    speakers.push(&b.speaker);
                }
            }
            // Confidence with two decimals (an f32 printed as f64 shows noise digits).
            let blocks: Vec<_> = blocks
                .iter()
                .map(|b| {
                    let words: Vec<_> = b
                        .words
                        .iter()
                        .map(|w| {
                            json!({
                                "text": w.text,
                                "start_ms": w.start_ms,
                                "end_ms": w.end_ms,
                                "conf": (f64::from(w.conf) * 100.0).round() / 100.0,
                            })
                        })
                        .collect();
                    json!({
                        "speaker": b.speaker,
                        "start_ms": b.start_ms,
                        "end_ms": b.end_ms,
                        "text": b.text,
                        "words": words,
                    })
                })
                .collect();
            let doc = json!({
                "title": title(e),
                "started_at": iso_utc(e.started_at),
                "duration_ms": e.duration_ms,
                "speakers": speakers,
                "blocks": blocks,
            });
            serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
        }
    }
}

/// A block as SRT cues of at most MAX_CUE_MS, split between words (one cue without words).
fn cues(b: &Block) -> Vec<(u32, u32, String)> {
    if b.words.is_empty() {
        return vec![(b.start_ms, b.end_ms.max(b.start_ms + 1), b.text.clone())];
    }
    let mut out = Vec::new();
    let mut start = b.words[0].start_ms;
    let mut text: Vec<&str> = Vec::new();
    let mut end = start;
    for w in &b.words {
        if !text.is_empty() && w.end_ms.saturating_sub(start) > MAX_CUE_MS {
            out.push((start, end, text.join(" ")));
            text.clear();
            start = w.start_ms;
        }
        text.push(&w.text);
        end = w.end_ms.max(end);
    }
    out.push((start, end.max(start + 1), text.join(" ")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn w(text: &str, start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: text.into(),
            start_ms,
            end_ms,
            conf: 0.9,
        }
    }

    fn part(speaker: &str, words: Vec<Word>) -> Part {
        Part {
            speaker: speaker.into(),
            start_ms: words[0].start_ms,
            end_ms: words.last().unwrap().end_ms,
            text: words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
        }
    }

    /// A short Dutch/English meeting: two people, "Me", a split block and an unnamed speaker.
    fn meeting() -> Export {
        let mut long = Vec::new();
        for (i, t) in
            "Ja, en dat is precies waarom we het budget voor volgend jaar opnieuw moeten bekijken."
                .split(' ')
                .enumerate()
        {
            long.push(w(t, 9000 + i as u32 * 700, 9000 + i as u32 * 700 + 600));
        }
        Export {
            title: "Kwartaaloverleg".into(),
            date_label: "31 okt 2026, 14:05".into(),
            started_at: 1_793_368_740_000,
            duration_ms: 3_725_000,
            parts: vec![
                part(
                    "Anna",
                    vec![w("Goedemorgen", 1200, 1800), w("allemaal.", 1800, 2300)],
                ),
                part(
                    "Anna",
                    vec![
                        w("Shall", 2600, 2900),
                        w("we", 2900, 3000),
                        w("start?", 3000, 3400),
                    ],
                ),
                part(
                    "Rob",
                    vec![
                        w("Yes,", 4000, 4300),
                        w("let's", 4300, 4600),
                        w("go.", 4600, 5000),
                    ],
                ),
                part("Speaker 3", long),
                part(
                    "Anna",
                    vec![
                        w("Eén", 3_600_000, 3_600_400),
                        w("vraag", 3_600_400, 3_601_000),
                        w("nog.", 3_601_000, 3_601_500),
                    ],
                ),
            ],
        }
    }

    /// Compare with `tests/snapshots/export.<ext>`; `UPDATE_SNAPSHOTS=1` rewrites them.
    fn snapshot(ext: &str, actual: &str) {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/snapshots/export.{ext}"));
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{} missing: run with UPDATE_SNAPSHOTS=1", path.display()));
        assert_eq!(
            actual, expected,
            "{ext} export changed (UPDATE_SNAPSHOTS=1 to accept)"
        );
    }

    #[test]
    fn markdown_snapshot() {
        snapshot("md", &render(&meeting(), Format::Markdown));
    }

    #[test]
    fn text_snapshot() {
        snapshot("txt", &render(&meeting(), Format::Text));
    }

    #[test]
    fn srt_snapshot() {
        snapshot("srt", &render(&meeting(), Format::Srt));
    }

    #[test]
    fn json_snapshot() {
        snapshot("json", &render(&meeting(), Format::Json));
    }

    #[test]
    fn blocks_merge_the_same_speaker_and_srt_cues_stay_short() {
        let b = blocks(&meeting().parts);
        assert_eq!(
            b.iter().map(|b| b.speaker.as_str()).collect::<Vec<_>>(),
            ["Anna", "Rob", "Speaker 3", "Anna"]
        );
        assert_eq!(b[0].text, "Goedemorgen allemaal. Shall we start?");
        let cues = cues(&b[2]);
        assert!(cues.len() >= 2, "a 10 s block is split");
        assert!(cues.iter().all(|(s, e, _)| e - s <= MAX_CUE_MS));
        let joined: Vec<_> = cues.iter().map(|c| c.2.as_str()).collect();
        assert_eq!(joined.join(" "), b[2].text, "no words lost or doubled");
    }

    #[test]
    fn speakers_are_named_as_in_the_app() {
        let seg = |id: i64, track: i64, speaker_id: Option<i64>, label: Option<&str>| Segment {
            id,
            meeting_id: 1,
            track_id: Some(track),
            pass: "final".into(),
            speaker_id,
            speaker_label: label.map(str::to_string),
            start_ms: id * 1000,
            end_ms: id * 1000 + 500,
            text: format!("t{id}"),
            words_json: Some(r#"[{"text":"t","start_ms":0,"end_ms":1,"conf":1.0}]"#.into()),
        };
        let track = |id: i64, kind: &str| Track {
            id,
            meeting_id: 1,
            source_kind: kind.into(),
            source_id: kind.into(),
            device_name: kind.into(),
            path: String::new(),
            seconds: 0.0,
        };
        let people = HashMap::from([(7, "Anna".to_string())]);
        let parts = parts(
            &[
                seg(1, 2, Some(7), Some("1")),
                seg(2, 2, None, Some("2")),
                seg(3, 1, None, Some("me")),
                seg(4, 1, None, None),
                seg(5, 2, None, None),
            ],
            &[track(1, "mic"), track(2, "loopback")],
            &people,
            "Rob",
        );
        let names: Vec<_> = parts.iter().map(|p| p.speaker.as_str()).collect();
        assert_eq!(names, ["Anna", "Speaker 2", "Rob", "Rob", "System audio"]);
        assert_eq!(parts[0].words.len(), 1, "words come along");
    }

    #[test]
    fn times_and_dates() {
        assert_eq!(clock(3_725_000), "1:02:05");
        assert_eq!(clock(65_000), "1:05");
        assert_eq!(srt_time(3_601_234), "01:00:01,234");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_793_368_740_000), "2026-10-30T13:59:00Z");
        assert_eq!(iso_utc(951_782_400_000), "2000-02-29T00:00:00Z", "leap day");
        assert_eq!(Format::parse("srt"), Some(Format::Srt));
        assert_eq!(Format::parse("docx"), None);
    }
}
