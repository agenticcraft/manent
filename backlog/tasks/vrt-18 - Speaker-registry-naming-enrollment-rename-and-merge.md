---
id: VRT-18
title: 'Speaker registry: naming, enrollment, rename and merge'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-31T20:27:00Z'
labels:
  - transcription
  - frontend
  - backend
milestone: m-2
dependencies:
  - VRT-17
  - VRT-30
documentation:
  - doc-2
  - doc-3
priority: high
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
App-owned speaker registry with in-app matching (validated in VRT-30/31/32): store per-speaker embeddings from the engine's speaker_profiles (diarize_profiles_pcm_json, VRT-15) as voice samples in SQLite (several per person), and match new speakers by cosine distance to the nearest sample with the decision rule in doc-2 §1.4 (auto-name <= 0.40 with a 0.15 margin to the next person, suggest <= 0.60, otherwise unnamed; slots under 5 s of clean speech never auto-named). Clicking a speaker chip and typing a name adds that speaker's embedding as a sample; confirmed/corrected names add more samples. Rename/merge apply retroactively. Speakers screen: rename, merge, delete voice data. No dependency on libparakeet's in-memory speaker registry. See doc-2 Technical architecture §1.4 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Naming a speaker updates all segments in the meeting
- [x] #2 Named speaker is recognized automatically after an app restart
- [x] #3 Two speakers can be merged
- [x] #4 Deleting a person removes their voice embedding
- [x] #5 Matching follows the doc-2 §1.4 decision rule (auto <= 0.40 with 0.15 margin, suggest <= 0.60) and ignores slots with < 5 s clean speech
- [x] #6 Unit tests cover cosine matching, the margin rule and the bands
- [x] #7 Works for Dutch and English speakers (naming is language-independent)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Migration 0002: meeting_voices(meeting_id, label, speaker_id, suggested_id, distance, embedding, encoder_id, clean_seconds, confirmed): a meeting's labels with their cleanest embedding and who they are. DB functions + tests; merge_speakers also moves meeting_voices.
2. speakers.rs: decide(embedding, clean_seconds, samples) per doc-2 §1.4 (nearest sample per person; auto <= 0.40 with 0.15 margin to the next person; suggest <= 0.60; < 5 s clean never auto, never a sample); unit tests for cosine, margin and bands. identify(meeting): match unconfirmed voices, apply speaker_id to the meeting's segments by label.
3. Passes deliver per-label voices (embedding, clean seconds, encoder id); rolling passes upsert them (keep the cleanest) and run identify; the final pass resets the meeting's voices, carries user-confirmed names over by overlap with the old segments, then identifies the rest.
4. Commands: meeting_voices, name_speaker (find or create the person, confirm, add a sample when >= 5 s clean, update segments), confirm_suggestion, merge_labels (two labels of a meeting are one person); people: list (meetings, samples), rename, merge, delete (voice data gone). Event speakers:changed.
5. UI: speaker chip popover on the Live screen (name with known people, confirm a suggestion, same as another speaker); chips show names and person colours; Speakers screen with rename, merge, delete.
6. Verify: unit tests; real app: AMI ES2004a (name the 4 people), restart the app, ES2004b (same people) -> auto-named correctly; Dutch interview: name in one part, recognised in a later part; merge and delete on the Speakers screen.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-31):
- Migration 0002 meeting_voices (a meeting's labels: cleanest embedding, person, suggestion, confirmed). DB: upsert (keeps the cleanest voice), identities, apply to segments, merge labels, clear; people list with meeting/sample counts; merge_speakers also moves meeting_voices. Tests.
- speakers.rs: decide() per doc-2 §1.4 (nearest sample per person, auto <= 0.40 with 0.15 margin, suggest <= 0.60, < 5 s clean never auto) + identify(); 4 unit tests (cosine, bands incl. edges, margin, nearest sample, short voices).
- Passes return per-label voices; rolling passes store + identify after every pass; the final pass clears the meeting's voices, carries user-confirmed names to the new labels by overlap (>= half), then identifies (unit-tested: names survive a renumbering).
- Commands: meeting_voices, name_speaker (existing person by name or new; sample when >= 5 s clean), confirm_suggestion, merge_labels, list_people, rename_person, merge_people, delete_person; event speakers:changed.
- UI: SpeakerChip with popover (name with known people, 'Is this X?' confirm, same person as another speaker), names + person colours on chips; Speakers screen (rename inline, merge via 'Same person as…', delete with confirmation and biometric note).
Verification (scripts/smoke-naming.py, real app):
- English: AMI ES2004a 5 min, 4 people named (first via the chip popover); final pass kept 388/390 words named; duplicate person merged on the Speakers screen; FEE016 deleted (person + samples gone); app restarted; ES2004b (same people): 421/421 words of known people auto-named, 0 wrong names, deleted person not recognised.
- Dutch: interview first half, 2 people named; restart; second half: 458/458 words auto-named (live, the guest stayed unnamed until the final pass: too little clean speech per rolling pass), 0 wrong.
- FEE016 had < 5 s clean speech, so no sample (by design). Not exercised on the real app: the 'Is this X?' confirm button (no suggestion-band case occurred); its command shares assign_person with naming.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Speaker naming and recognition: a meeting's speaker labels keep their voice (meeting_voices) and are matched against the app's own registry with the doc-2 §1.4 rule after every pass; naming a chip adds a voice sample and renames every segment; names survive the final pass. Speakers screen with rename, merge and delete (voice data removed). Verified on the real app: people named in one meeting are recognised after an app restart in a later meeting (English AMI 421/421 words, Dutch interview 458/458, 0 wrong names), a deleted person is not recognised.
<!-- SECTION:FINAL_SUMMARY:END -->
