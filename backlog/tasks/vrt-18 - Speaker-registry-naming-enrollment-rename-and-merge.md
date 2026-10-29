---
id: VRT-18
title: 'Speaker registry: naming, enrollment, rename and merge'
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T17:05:00Z'
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
App-owned speaker registry with in-app matching (validated in VRT-30): store per-speaker embeddings from /v1/audio/diarization include_speaker_profiles as voice samples in SQLite (several per person), match new speakers by cosine distance to the nearest sample with three bands (<= 0.45 auto-name, 0.45-0.60 suggest, > 0.60 unnamed). Clicking a speaker chip and typing a name adds that speaker's embedding as a sample; confirmed/corrected names add more samples. Rename/merge apply retroactively. Speakers screen: rename, merge, delete voice data. No dependency on LocalAI's in-memory voice registry. See doc-2 Technical architecture §1.4 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Naming a speaker updates all segments in the meeting
- [ ] #2 Named speaker is recognized automatically after an app restart
- [ ] #3 Two speakers can be merged
- [ ] #4 Deleting a person removes their voice embedding
- [ ] #5 Matching uses the nearest stored sample with the doc-2 §1.4 decision bands; suggestions in the 0.45-0.60 band need user confirmation
- [ ] #6 Unit tests cover cosine matching and the decision bands
- [ ] #7 Works for Dutch and English speakers (naming is language-independent)
<!-- AC:END -->
