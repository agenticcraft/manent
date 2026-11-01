---
id: VRT-37
title: Transcript as Markdown next to the meeting's audio
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T11:41:00Z'
updated_date: '2026-11-01T16:49:00Z'
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
- [x] #1 Every transcribed meeting has transcript.md next to its audio: YAML frontmatter (versioned) plus the Markdown export body
- [x] #2 Rewritten atomically after the final pass, re-transcribe, naming, merge and title changes
- [x] #3 Contains no voice embeddings
- [x] #4 Round-trip test (write, parse, same blocks and header); snapshot test; existing meetings backfilled
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built:
- **Module:** src-tauri/src/transcript_md.rs.
  - render writes the YAML header and the body. Every header value is JSON, which is valid YAML: PyYAML reads the snapshot header. So no YAML crate is needed.
  - parse reads it back, strictly: the schema version must not be newer, and the header must be complete.
  - load is shared with export_meeting.
- **Writing:** after every transcription job (final pass, import, re-transcribe), after drop_tracks so the audio list is right.
- **Rewriting:**
  - in the background on rename_meeting and on name/confirm/merge of a speaker;
  - for all meetings on rename/merge/delete of a person and on set_me_name.
- **Not written during recording:** rolling passes don't write it.
- **Backfill:** at startup for done meetings without the file.
- **Body:** gaps (VRT-36) are in the header and appear as a '> No audio …' note at their place in the body.
- **Tests:**
  - unit: snapshot (tests/snapshots/transcript.md), round trip, gap placement, rejects, pass.
  - real app: scripts/smoke-transcript-md.py PASS. The backfill wrote 7 of 7 files. A 25-min live recording gave 207 blocks, and its word count equals the app's (6687). Renaming, naming a speaker and changing the user's own name each rewrite the file.
- **Checks:** 112 Rust tests, clippy 0, npm check green. doc-2 updated (storage).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every meeting folder now has transcript.md: Markdown with a versioned YAML header, kept up to date on every change and readable back by parse. It is the basis for Nextcloud sync and the companion app.
<!-- SECTION:FINAL_SUMMARY:END -->
