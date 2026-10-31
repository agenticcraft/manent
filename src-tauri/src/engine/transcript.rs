//! What the engine returns, parsed from the helper's JSON, and the word/speaker merge
//! (doc-2 §1.3: each word gets the speaker of the diarization turn it overlaps most).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One recognised word. Times are milliseconds from the start of the audio sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    /// Model confidence in (0, 1].
    pub conf: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transcript {
    pub text: String,
    pub words: Vec<Word>,
}

/// A stretch where diarization hears one speaker (slot numbers are per call).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpeakerTurn {
    pub speaker: u32,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// One clean voice embedding per usable speaker slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerProfile {
    pub speaker: u32,
    /// Seconds of clean (single-speaker) speech behind the embedding (capped at 30 by the engine).
    pub clean_seconds: f32,
    /// L2-normalised embedding, or None when the speaker had too little clean speech.
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diarization {
    pub turns: Vec<SpeakerTurn>,
    pub profiles: Vec<SpeakerProfile>,
    /// `sha256:` identity of the speaker encoder; embeddings only compare within one encoder.
    pub encoder_id: String,
}

/// Consecutive words of one speaker: what the transcript shows as a block.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Segment {
    pub speaker: Option<u32>,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

fn ms(v: &Value) -> u32 {
    (v.as_f64().unwrap_or(0.0).max(0.0) * 1000.0).round() as u32
}

/// `{"text","words":[{"w","start","end","conf"}]}` from the helper's `transcribe`.
pub fn parse_transcript(doc: &Value) -> Result<Transcript, String> {
    let words = doc["words"]
        .as_array()
        .ok_or("transcript without words")?
        .iter()
        .map(|w| Word {
            text: w["w"].as_str().unwrap_or_default().to_string(),
            start_ms: ms(&w["start"]),
            end_ms: ms(&w["end"]),
            conf: w["conf"].as_f64().unwrap_or(0.0) as f32,
        })
        .filter(|w| !w.text.is_empty())
        .collect();
    Ok(Transcript {
        text: doc["text"].as_str().unwrap_or_default().trim().to_string(),
        words,
    })
}

/// The helper's `diarize` document: segments plus speaker_profiles v1.
pub fn parse_diarization(doc: &Value) -> Result<Diarization, String> {
    let turns = doc["segments"]
        .as_array()
        .ok_or("diarization without segments")?
        .iter()
        .map(|s| SpeakerTurn {
            speaker: s["speaker"].as_u64().unwrap_or(0) as u32,
            start_ms: ms(&s["start"]),
            end_ms: ms(&s["end"]),
        })
        .collect();
    let sp = &doc["speaker_profiles"];
    let profiles = sp["speakers"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|p| SpeakerProfile {
            speaker: p["speaker"].as_u64().unwrap_or(0) as u32,
            clean_seconds: p["clean_duration"].as_f64().unwrap_or(0.0) as f32,
            embedding: p["embedding"]
                .as_array()
                .map(|e| e.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()),
        })
        .collect();
    Ok(Diarization {
        turns,
        profiles,
        encoder_id: sp["encoder"]["identity"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    })
}

/// A new block starts when the speaker changes or after a pause this long.
const BLOCK_GAP_MS: u32 = 1500;

/// The speaker of the turn a word overlaps most; without overlap the nearest turn.
fn speaker_of(word: &Word, turns: &[SpeakerTurn]) -> Option<u32> {
    let overlap =
        |t: &SpeakerTurn| word.end_ms.min(t.end_ms) as i64 - word.start_ms.max(t.start_ms) as i64;
    if let Some(t) = turns
        .iter()
        .filter(|t| overlap(t) > 0)
        .max_by_key(|t| overlap(t))
    {
        return Some(t.speaker);
    }
    let mid = (word.start_ms + word.end_ms) / 2;
    turns
        .iter()
        .min_by_key(|t| {
            if mid < t.start_ms {
                t.start_ms - mid
            } else {
                mid.saturating_sub(t.end_ms)
            }
        })
        .map(|t| t.speaker)
}

/// Words plus diarization -> speaker blocks.
pub fn merge(words: &[Word], turns: &[SpeakerTurn]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for w in words {
        let speaker = speaker_of(w, turns);
        match out.last_mut() {
            Some(seg)
                if seg.speaker == speaker
                    && w.start_ms.saturating_sub(seg.end_ms) < BLOCK_GAP_MS =>
            {
                seg.end_ms = seg.end_ms.max(w.end_ms);
                seg.words.push(w.clone());
            }
            _ => out.push(Segment {
                speaker,
                start_ms: w.start_ms,
                end_ms: w.end_ms,
                text: String::new(),
                words: vec![w.clone()],
            }),
        }
    }
    for seg in &mut out {
        seg.text = seg
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn word(text: &str, start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: text.into(),
            start_ms,
            end_ms,
            conf: 1.0,
        }
    }

    fn turn(speaker: u32, start_ms: u32, end_ms: u32) -> SpeakerTurn {
        SpeakerTurn {
            speaker,
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn parses_the_helper_documents() {
        let t = parse_transcript(&json!({
            "text": " Hallo wereld. ", "frame_sec": 0.08,
            "words": [{"w": "Hallo", "start": 0.48, "end": 0.8, "conf": 0.91}, {"w": "wereld.", "start": 0.8, "end": 1.2, "conf": 1.0}]
        }))
        .unwrap();
        assert_eq!(t.text, "Hallo wereld.");
        assert_eq!(
            t.words[0],
            Word {
                text: "Hallo".into(),
                start_ms: 480,
                end_ms: 800,
                conf: 0.91
            }
        );

        let d = parse_diarization(&json!({
            "segments": [{"speaker": 1, "start": 1.83, "end": 4.29}],
            "speaker_profiles": {"version": 1, "encoder": {"identity": "sha256:abc", "dimension": 2},
              "speakers": [{"speaker": 1, "clean_duration": 9.16, "embedding": [0.6, 0.8]},
                           {"speaker": 2, "clean_duration": 0.5, "embedding": null, "unavailable_reason": "too short"}]}
        }))
        .unwrap();
        assert_eq!(d.turns, vec![turn(1, 1830, 4290)]);
        assert_eq!(d.encoder_id, "sha256:abc");
        assert_eq!(d.profiles[0].embedding.as_deref(), Some(&[0.6f32, 0.8][..]));
        assert_eq!(d.profiles[1].embedding, None);
        assert!(parse_transcript(&json!({"text": "x"})).is_err());
    }

    #[test]
    fn words_take_the_speaker_they_overlap_most() {
        let turns = [turn(0, 0, 2000), turn(1, 1900, 4000)];
        let words = [
            word("a", 100, 500),
            word("b", 1800, 2300),
            word("c", 3000, 3500),
        ];
        let segs = merge(&words, &turns);
        assert_eq!(segs.len(), 2);
        assert_eq!((segs[0].speaker, segs[0].text.as_str()), (Some(0), "a"));
        // "b" overlaps speaker 0 for 200 ms and speaker 1 for 400 ms.
        assert_eq!((segs[1].speaker, segs[1].text.as_str()), (Some(1), "b c"));
        assert_eq!((segs[1].start_ms, segs[1].end_ms), (1800, 3500));
    }

    #[test]
    fn words_outside_every_turn_take_the_nearest_speaker() {
        let turns = [turn(0, 0, 1000), turn(1, 5000, 6000)];
        let segs = merge(
            &[word("early", 1200, 1400), word("late", 4500, 4800)],
            &turns,
        );
        assert_eq!(
            segs.iter().map(|s| s.speaker).collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );
    }

    #[test]
    fn a_long_pause_starts_a_new_block_and_no_diarization_means_no_speaker() {
        let segs = merge(
            &[
                word("one", 0, 400),
                word("two", 500, 900),
                word("three", 3000, 3400),
            ],
            &[],
        );
        assert_eq!(segs.len(), 2);
        assert_eq!((segs[0].speaker, segs[0].text.as_str()), (None, "one two"));
        assert_eq!(segs[1].text, "three");
        assert!(merge(&[], &[turn(0, 0, 10)]).is_empty());
    }
}
