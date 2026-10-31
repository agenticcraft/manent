//! Speaker registry and in-app voice matching (doc-2 §1.4, VRT-18). People and their voice
//! samples live in SQLite; a meeting's speaker labels are matched against them by cosine
//! distance to each person's nearest sample.

use std::collections::HashMap;

use crate::db::{Db, VoiceSample};

/// Auto-name when the nearest person is this close...
pub const AUTO: f32 = 0.40;
/// ...and at least this much closer than any other person.
pub const MARGIN: f32 = 0.15;
/// Suggest the nearest name (the user confirms) up to this distance.
pub const SUGGEST: f32 = 0.60;
/// Voices with less clean speech are never auto-named and never become samples.
pub const MIN_CLEAN_SECONDS: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Match {
    Auto { speaker_id: i64, distance: f32 },
    Suggest { speaker_id: i64, distance: f32 },
    Unknown,
}

pub fn cos_dist(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - dot / (na * nb).max(f32::EPSILON)
}

/// The decision rule of doc-2 §1.4 for one voice against everyone's samples (same encoder).
pub fn decide(embedding: &[f32], clean_seconds: f64, samples: &[VoiceSample]) -> Match {
    // Nearest sample per person.
    let mut nearest: HashMap<i64, f32> = HashMap::new();
    for s in samples
        .iter()
        .filter(|s| s.embedding.len() == embedding.len())
    {
        let d = cos_dist(embedding, &s.embedding);
        nearest
            .entry(s.speaker_id)
            .and_modify(|best| *best = best.min(d))
            .or_insert(d);
    }
    let mut ranked: Vec<(i64, f32)> = nearest.into_iter().collect();
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
    let Some(&(speaker_id, distance)) = ranked.first() else {
        return Match::Unknown;
    };
    let runner_up = ranked.get(1).map_or(f32::INFINITY, |r| r.1);
    if distance <= AUTO && runner_up - distance >= MARGIN && clean_seconds >= MIN_CLEAN_SECONDS {
        Match::Auto {
            speaker_id,
            distance,
        }
    } else if distance <= SUGGEST {
        Match::Suggest {
            speaker_id,
            distance,
        }
    } else {
        Match::Unknown
    }
}

/// Match a meeting's voices that the user hasn't named against the registry, and give every
/// segment of the meeting the person its label stands for.
pub async fn identify(db: &Db, meeting_id: i64) -> Result<(), String> {
    let err = |e: crate::db::DbError| e.to_string();
    let voices = db.meeting_voices(meeting_id).await.map_err(err)?;
    let mut samples_by_encoder: HashMap<String, Vec<VoiceSample>> = HashMap::new();
    for v in voices.iter().filter(|v| !v.confirmed) {
        let (Some(embedding), Some(encoder)) = (&v.embedding, &v.encoder_id) else {
            continue;
        };
        if !samples_by_encoder.contains_key(encoder) {
            let samples = db.voice_samples(encoder).await.map_err(err)?;
            samples_by_encoder.insert(encoder.clone(), samples);
        }
        let (speaker, suggested, distance) =
            match decide(embedding, v.clean_seconds, &samples_by_encoder[encoder]) {
                Match::Auto {
                    speaker_id,
                    distance,
                } => (Some(speaker_id), None, Some(distance)),
                Match::Suggest {
                    speaker_id,
                    distance,
                } => (None, Some(speaker_id), Some(distance)),
                Match::Unknown => (None, None, None),
            };
        db.set_voice_identity(
            meeting_id,
            &v.label,
            speaker,
            suggested,
            distance.map(f64::from),
            false,
        )
        .await
        .map_err(err)?;
    }
    db.apply_voice_identities(meeting_id).await.map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(speaker_id: i64, e: &[f32]) -> VoiceSample {
        VoiceSample {
            id: 0,
            speaker_id,
            embedding: e.to_vec(),
            clean_seconds: 10.0,
        }
    }

    /// A unit vector at `distance` (cosine) from (1, 0, 0), in the x-y plane.
    fn at(distance: f32) -> Vec<f32> {
        let c = 1.0 - distance;
        vec![c, (1.0 - c * c).sqrt(), 0.0]
    }

    #[test]
    fn cosine_distance() {
        assert!(
            cos_dist(&[1.0, 0.0], &[2.0, 0.0]).abs() < 1e-6,
            "length doesn't matter"
        );
        assert!((cos_dist(&[1.0, 0.0], &[0.0, 3.0]) - 1.0).abs() < 1e-6);
        assert!((cos_dist(&[1.0, 0.0], &[-1.0, 0.0]) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn bands_auto_suggest_unknown() {
        let anna = [sample(1, &at(0.0))];
        assert!(matches!(
            decide(&at(0.30), 10.0, &anna),
            Match::Auto { speaker_id: 1, .. }
        ));
        assert!(
            matches!(decide(&at(0.40), 10.0, &anna), Match::Auto { .. }),
            "0.40 inclusive"
        );
        assert!(matches!(
            decide(&at(0.45), 10.0, &anna),
            Match::Suggest { speaker_id: 1, .. }
        ));
        assert!(
            matches!(decide(&at(0.60), 10.0, &anna), Match::Suggest { .. }),
            "0.60 inclusive"
        );
        assert_eq!(decide(&at(0.70), 10.0, &anna), Match::Unknown);
        assert_eq!(
            decide(&at(0.10), 10.0, &[]),
            Match::Unknown,
            "nobody enrolled"
        );
    }

    #[test]
    fn auto_needs_a_margin_to_the_next_person() {
        // Anna at 0.20, Bram at 0.30: too close to tell -> only a suggestion.
        let probe = [1.0, 0.0, 0.0];
        let rotate = |d: f32, z: f32| {
            let c = 1.0 - d;
            vec![
                c,
                (1.0 - c * c).sqrt() * (1.0 - z * z).sqrt(),
                (1.0 - c * c).sqrt() * z,
            ]
        };
        let samples = [sample(1, &rotate(0.20, 0.0)), sample(2, &rotate(0.30, 1.0))];
        assert!(matches!(
            decide(&probe, 10.0, &samples),
            Match::Suggest { speaker_id: 1, .. }
        ));
        // Bram at 0.40 instead: the margin is 0.20 -> auto.
        let samples = [sample(1, &rotate(0.20, 0.0)), sample(2, &rotate(0.40, 1.0))];
        assert!(matches!(
            decide(&probe, 10.0, &samples),
            Match::Auto { speaker_id: 1, .. }
        ));
    }

    #[test]
    fn the_nearest_sample_of_each_person_counts_and_short_voices_are_never_auto() {
        let samples = [
            sample(1, &at(0.9)),
            sample(1, &at(0.1)),
            sample(2, &at(0.8)),
        ];
        assert_eq!(
            decide(&at(0.0), 10.0, &samples),
            Match::Auto {
                speaker_id: 1,
                distance: cos_dist(&at(0.0), &at(0.1))
            }
        );
        assert!(
            matches!(
                decide(&at(0.0), 4.0, &samples),
                Match::Suggest { speaker_id: 1, .. }
            ),
            "under 5 s of clean speech: a suggestion at most"
        );
    }
}
