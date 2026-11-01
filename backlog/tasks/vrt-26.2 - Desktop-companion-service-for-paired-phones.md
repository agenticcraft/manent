---
id: VRT-26.2
title: 'Desktop: companion service for paired phones'
status: To Do
assignee: []
created_date: '2026-11-01T10:07:00Z'
labels:
  - mobile
milestone: m-4
dependencies:
  - VRT-26.1
parent_task_id: VRT-26
priority: low
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Desktop side of the companion (decision-4), following the transport decision of VRT-26.1. Settings gets a Companion section: off by default; 'Pair a phone' shows a QR code; paired phones are listed and can be revoked. While enabled, the service listens on the local network for paired devices only. It accepts recordings and turns each one into a meeting through the import job (VRT-19), so it is transcribed, speakers are labelled and known people recognised. It tells the phone when the transcript is ready. Uploads resume after an interruption; an upload from a revoked or unknown device is refused.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Off by default; pairing by QR code; paired phones listed and revocable
- [ ] #2 A recording uploaded by a paired phone becomes a transcribed meeting (import job)
- [ ] #3 Unknown or revoked devices are refused; interrupted uploads resume
<!-- AC:END -->
