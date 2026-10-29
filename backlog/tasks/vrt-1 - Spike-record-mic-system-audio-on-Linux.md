---
id: VRT-1
title: 'Spike: record mic + system audio on Linux'
status: To Do
assignee: []
created_date: '2026-10-29 12:16'
updated_date: '2026-10-29 12:19'
labels:
  - spike
  - audio
  - linux
milestone: m-0
dependencies: []
documentation:
  - doc-2
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Prove we can capture the microphone and the system output (PipeWire sink monitor) simultaneously on Fedora/PipeWire from Rust, as separate streams. De-risks the core of stage 1. See doc-2 Technical architecture §1.2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust CLI lists input devices and output monitors
- [ ] #2 CLI records mic and system audio concurrently into two WAV files
- [ ] #3 Both files play back correctly and stay in sync over 5 minutes
- [ ] #4 Findings (crate choice: cpal vs pipewire-rs) written to implementation notes
<!-- AC:END -->
