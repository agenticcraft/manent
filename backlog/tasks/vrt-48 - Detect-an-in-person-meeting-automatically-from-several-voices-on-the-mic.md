---
id: VRT-48
title: Detect an in-person meeting automatically from several voices on the mic
status: To Do
assignee: []
created_date: '2026-11-04T12:27:00Z'
labels:
  - speakers
milestone: m-4
dependencies: []
priority: low
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to the in-person switch on the Live screen: the user has to remember to switch to "In person". When the rolling passes hear several distinct voices on the mic track (with no matching speech on the system track, so not bleed), Manent could switch, or suggest switching, to in-person by itself. Needs a short spike: is the mic diarization reliable enough to tell one voice from several (noise, a headset, the user's own voice changing), and how much extra CPU diarizing the mic in every online meeting costs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A spike measures how reliably several voices on the mic are detected (real in-person and online recordings), and the CPU cost of diarizing the mic in online meetings; findings in the implementation notes
- [ ] #2 When several voices are detected on the mic during an online recording, the Live screen suggests switching to in person (or switches, per the spike's outcome), and the final pass treats the whole meeting accordingly
- [ ] #3 No false switch in an online meeting with bleed through the speakers (smoke or fixture test)
<!-- AC:END -->
