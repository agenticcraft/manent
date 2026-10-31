//! Speaker labels for the rolling and final passes (VRT-17, doc-2 §1.2-§1.4).
//!
//! The mic track is "Me". The system track is diarized; its speaker slots are per call, so a
//! rolling pass maps them to meeting labels ("1", "2", ...) by continuity with the words it
//! already labelled, then by voice embedding, and otherwise starts a new label. The mic also
//! hears the speakers (bleed): the system track wins where both tracks say the same words.

use std::collections::HashMap;

use crate::engine::transcript::{self, Diarization, SpeakerTurn, Word};

/// Label of the mic track's speaker.
pub const ME: &str = "me";
/// Two embeddings closer than this are the same person (doc-2 §1.4 auto-name band).
const SAME_VOICE: f32 = 0.40;
/// Mic and system words this close in time with the same text are one utterance.
const BLEED_WINDOW_MS: i64 = 1000;
/// Labelled system words are remembered this long for continuity between passes.
const MEMORY_MS: u32 = 5 * 60_000;
/// A slot takes an earlier label by continuity when it overlaps that label's words this much.
const CONTINUITY_MS: u32 = 500;

/// A transcript block with its meeting label, ready to store.
#[derive(Debug, Clone, PartialEq)]
pub struct Labelled {
    pub label: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    pub words: Vec<Word>,
}

fn norm(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn mid(w: &Word) -> i64 {
    (w.start_ms as i64 + w.end_ms as i64) / 2
}

/// Drop mic speech that is the system audio heard through the speakers. A mic block (words
/// without a 1.5 s pause) goes when at least half its words match a system word (same text
/// within 1 s); otherwise only the matching words go. When the user and a remote speaker
/// really say the same word at the same moment, the system track keeps it.
pub fn remove_bleed(mic: &[Word], system: &[Word]) -> Vec<Word> {
    let echoed = |w: &Word| {
        let text = norm(&w.text);
        !text.is_empty()
            && system
                .iter()
                .any(|s| (mid(s) - mid(w)).abs() <= BLEED_WINDOW_MS && norm(&s.text) == text)
    };
    let mut kept = Vec::new();
    for block in transcript::merge(mic, &[]) {
        let flags: Vec<bool> = block.words.iter().map(echoed).collect();
        let n_echoed = flags.iter().filter(|&&e| e).count();
        if n_echoed * 2 >= block.words.len() {
            continue;
        }
        kept.extend(
            block
                .words
                .into_iter()
                .zip(flags)
                .filter(|(_, e)| !e)
                .map(|(w, _)| w),
        );
    }
    kept
}

/// Blocks of the mic track, all "Me".
pub fn me_blocks(words: &[Word]) -> Vec<Labelled> {
    transcript::merge(words, &[])
        .into_iter()
        .map(|s| Labelled {
            label: ME.into(),
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text,
            words: s.words,
        })
        .collect()
}

/// Speaker blocks of the system track with `labels` per diarization slot (words without a
/// labelled slot are left out).
pub fn system_blocks(
    words: &[Word],
    turns: &[SpeakerTurn],
    labels: &HashMap<u32, String>,
) -> Vec<Labelled> {
    transcript::merge(words, turns)
        .into_iter()
        .filter_map(|s| {
            let label = labels.get(&s.speaker?)?.clone();
            Some(Labelled {
                label,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                text: s.text,
                words: s.words,
            })
        })
        .collect()
}

/// Final pass: one diarization of the whole meeting, so slots are consistent; number them
/// in the order they first speak.
pub fn number_by_appearance(words: &[Word], turns: &[SpeakerTurn]) -> HashMap<u32, String> {
    let mut labels = HashMap::new();
    for seg in transcript::merge(words, turns) {
        if let Some(slot) = seg.speaker {
            let n = labels.len() + 1;
            labels.entry(slot).or_insert_with(|| n.to_string());
        }
    }
    labels
}

fn cos_dist(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - dot / (na * nb).max(f32::EPSILON)
}

/// What the rolling passes of one recording remember about its speakers.
#[derive(Default)]
pub struct RollingLabels {
    next: u32,
    /// Best embedding per label (the one with the most clean speech).
    voices: Vec<(String, Vec<f32>, f32)>,
    /// System words already labelled (meeting ms), for continuity.
    recent: Vec<(u32, u32, String)>,
}

impl RollingLabels {
    /// Labels for the slots of one rolling pass. `diar` and `words` are in meeting ms; only
    /// slots with words in `kept` get a new label when nothing else matches.
    pub fn assign(&mut self, diar: &Diarization, kept: &[Word]) -> HashMap<u32, String> {
        let slot_at = |start: u32, end: u32| {
            diar.turns
                .iter()
                .map(|t| {
                    (
                        t.speaker,
                        end.min(t.end_ms).saturating_sub(start.max(t.start_ms)),
                    )
                })
                .filter(|&(_, o)| o > 0)
                .max_by_key(|&(_, o)| o)
                .map(|(s, _)| s)
        };
        // (a) Continuity: earlier labelled words heard again in this diarization's context.
        let mut tally: HashMap<(u32, String), u32> = HashMap::new();
        for (start, end, label) in &self.recent {
            if let Some(slot) = slot_at(*start, *end) {
                *tally.entry((slot, label.clone())).or_default() += end - start;
            }
        }
        let mut labels: HashMap<u32, String> = HashMap::new();
        let mut slots: Vec<u32> = diar.turns.iter().map(|t| t.speaker).collect();
        slots.sort_unstable();
        slots.dedup();
        for &slot in &slots {
            let best = tally
                .iter()
                .filter(|((s, _), _)| *s == slot)
                .max_by_key(|(_, ms)| **ms);
            if let Some(((_, label), ms)) = best
                && *ms >= CONTINUITY_MS
            {
                labels.insert(slot, label.clone());
            }
        }
        // (b) Voice: the nearest earlier label within the same-person distance.
        for p in &diar.profiles {
            if labels.contains_key(&p.speaker) {
                continue;
            }
            let Some(e) = &p.embedding else { continue };
            let nearest = self
                .voices
                .iter()
                .map(|(l, v, _)| (l, cos_dist(e, v)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((label, d)) = nearest
                && d <= SAME_VOICE
            {
                labels.insert(p.speaker, label.clone());
            }
        }
        // (c) A new speaker, if the slot says something in this pass (numbered in the order
        // they speak).
        for w in kept {
            if let Some(slot) = slot_at(w.start_ms, w.end_ms.max(w.start_ms + 1)) {
                labels.entry(slot).or_insert_with(|| {
                    self.next += 1;
                    self.next.to_string()
                });
            }
        }
        // Remember the voices (keep the cleanest embedding per label) and the kept words.
        for p in &diar.profiles {
            let (Some(label), Some(e)) = (labels.get(&p.speaker), &p.embedding) else {
                continue;
            };
            match self.voices.iter_mut().find(|(l, _, _)| l == label) {
                Some(v) if v.2 >= p.clean_seconds => {}
                Some(v) => *v = (label.clone(), e.clone(), p.clean_seconds),
                None => self
                    .voices
                    .push((label.clone(), e.clone(), p.clean_seconds)),
            }
        }
        for w in kept {
            if let Some(label) =
                slot_at(w.start_ms, w.end_ms.max(w.start_ms + 1)).and_then(|s| labels.get(&s))
            {
                self.recent
                    .push((w.start_ms, w.end_ms.max(w.start_ms + 1), label.clone()));
            }
        }
        if let Some(&(_, last_end, _)) = self.recent.last() {
            self.recent
                .retain(|(_, end, _)| end + MEMORY_MS >= last_end);
        }
        labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::transcript::SpeakerProfile;

    fn w(text: &str, start_ms: u32, end_ms: u32) -> Word {
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

    /// A voice embedding (normalised by the distance function).
    fn voice(speaker: u32, e: [f32; 3]) -> SpeakerProfile {
        SpeakerProfile {
            speaker,
            clean_seconds: 10.0,
            embedding: Some(e.to_vec()),
        }
    }

    fn diar(turns: Vec<SpeakerTurn>, profiles: Vec<SpeakerProfile>) -> Diarization {
        Diarization {
            turns,
            profiles,
            encoder_id: "sha256:x".into(),
        }
    }

    #[test]
    fn mic_blocks_that_echo_the_system_track_are_dropped() {
        let system = [
            w("Good", 1000, 1300),
            w("morning,", 1300, 1700),
            w("everyone.", 1700, 2300),
        ];
        let mic = [
            // The speakers, heard by the mic a little later and partly misheard.
            w("good", 1100, 1400),
            w("morning", 1400, 1800),
            w("every", 1800, 2100),
            w("one", 2100, 2400),
            // The user, after a pause.
            w("Morning!", 5000, 5500),
            w("Shall", 5600, 5800),
            w("we", 5800, 5900),
            w("start?", 5900, 6300),
        ];
        let kept: Vec<_> = remove_bleed(&mic, &system)
            .into_iter()
            .map(|w| w.text)
            .collect();
        assert_eq!(kept, ["Morning!", "Shall", "we", "start?"]);
    }

    #[test]
    fn the_user_talking_over_someone_keeps_their_own_words() {
        let system = [
            w("so", 1000, 1200),
            w("the", 1200, 1300),
            w("budget", 1300, 1800),
        ];
        let mic = [
            w("sorry", 1100, 1500),
            w("the", 1500, 1600),
            w("budget", 1600, 2000),
            w("is", 2000, 2100),
            w("fine", 2100, 2500),
            w("now", 2500, 2800),
        ];
        let kept: Vec<_> = remove_bleed(&mic, &system)
            .into_iter()
            .map(|w| w.text)
            .collect();
        assert_eq!(
            kept,
            ["sorry", "is", "fine", "now"],
            "only the echoed words go"
        );
        assert_eq!(remove_bleed(&mic, &[]).len(), mic.len());
    }

    #[test]
    fn final_labels_follow_the_order_people_first_speak() {
        let turns = [turn(3, 0, 1000), turn(0, 1000, 2000), turn(3, 2000, 3000)];
        let words = [w("a", 100, 400), w("b", 1100, 1400), w("c", 2100, 2400)];
        let labels = number_by_appearance(&words, &turns);
        assert_eq!((labels[&3].as_str(), labels[&0].as_str()), ("1", "2"));
        let blocks = system_blocks(&words, &turns, &labels);
        assert_eq!(
            blocks.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(),
            ["1", "2", "1"]
        );
        assert_eq!(me_blocks(&words)[0].label, ME);
    }

    #[test]
    fn rolling_passes_keep_labels_by_continuity_and_by_voice() {
        let mut state = RollingLabels::default();
        // Pass 1: two people.
        let d1 = diar(
            vec![turn(0, 0, 4000), turn(1, 4000, 8000)],
            vec![voice(0, [1.0, 0.0, 0.0]), voice(1, [0.0, 1.0, 0.0])],
        );
        let k1 = [w("hi", 500, 1500), w("hello", 4500, 5500)];
        let l1 = state.assign(&d1, &k1);
        assert_eq!((l1[&0].as_str(), l1[&1].as_str()), ("1", "2"));
        // Pass 2 (context 6-8 s overlaps pass 1): slot numbers swapped by the diarizer.
        // Slot 0 now covers the earlier "hello" -> label 2 by continuity; slot 1 is a voice
        // close to label 1's -> label 1; slot 2 is new.
        let d2 = diar(
            vec![
                turn(0, 4000, 8000),
                turn(1, 8000, 12000),
                turn(2, 12000, 16000),
            ],
            // Slot 1 is close to label 1's voice; slot 2 is far from both.
            vec![voice(1, [0.98, 0.1, 0.1]), voice(2, [0.1, 0.1, 1.0])],
        );
        let k2 = [
            w("so", 8500, 9000),
            w("yes", 12500, 13000),
            w("and", 6000, 7000),
        ];
        let l2 = state.assign(&d2, &k2);
        assert_eq!(l2[&0], "2", "continuity");
        assert_eq!(l2[&1], "1", "same voice");
        assert_eq!(l2[&2], "3", "a new speaker");
    }

    #[test]
    fn a_slot_without_words_or_voice_gets_no_new_label() {
        let mut state = RollingLabels::default();
        let d = diar(vec![turn(0, 0, 1000), turn(1, 2000, 3000)], vec![]);
        let labels = state.assign(&d, &[w("only", 100, 500)]);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[&0], "1");
    }
}
