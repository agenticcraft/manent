---
id: VRT-37
title: Transcript as Markdown next to the meeting's audio
status: To Do
assignee: []
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-01T11:55:00Z'
labels:
  - transcript
  - storage
milestone: m-4
dependencies: []
priority: medium
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-5: every meeting folder gets a transcript.md beside its audio, so the result can be read and searched without the database: by the companion app through Nextcloud (VRT-26.4), in Nextcloud's web viewer and search, in any Markdown editor, and by anyone who opens the meetings folder.
Format: the Markdown export of VRT-21, made strict enough to parse back, with YAML frontmatter for the structured part:
---
veritas: 1            # schema version
id: 42
title: Kwartaaloverleg
started_at: 2026-10-31T14:05:00+01:00
duration_s: 3725
audio: [mix.opus]     # the file(s) it belongs to (or the phone's recording)
pass: final
speakers: [Anna, Rob, Speaker 3]
---
then per block '**Name** · 0:01' and the text (timestamps let the phone seek per block).
It holds speaker names and labels only, never voice embeddings (biometric data stays in the database). Word-level timings are not in it; if word highlighting on the phone needs them later, an optional transcript.words.json sidecar. Written atomically (temp file + rename) after the final pass and rewritten when a speaker is named, merged or renamed, the title changes or the meeting is re-transcribed; deleting a meeting removes it with the folder. Existing meetings get one on first start (backfill job). A parser for the format lives next to the writer (round-trip test), for the phone and for later re-import.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every transcribed meeting has transcript.md next to its audio: YAML frontmatter (versioned) plus the Markdown export body
- [ ] #2 Rewritten atomically after the final pass, re-transcribe, naming, merge and title changes
- [ ] #3 Contains no voice embeddings
- [ ] #4 Round-trip test (write, parse, same blocks and header); snapshot test; existing meetings backfilled
<!-- AC:END -->
