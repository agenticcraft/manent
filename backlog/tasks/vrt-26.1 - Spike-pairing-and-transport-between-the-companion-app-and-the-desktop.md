---
id: VRT-26.1
title: 'Spike: pairing and transport between the companion app and the desktop'
status: To Do
assignee: []
created_date: '2026-11-01T10:07:00Z'
labels:
  - spike
  - mobile
milestone: m-4
dependencies: []
parent_task_id: VRT-26
priority: low
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decide how a phone and a desktop Veritas find each other, pair, and exchange recordings and transcripts privately (decision-4). Questions:
- Discovery on the local network (mDNS/DNS-SD), and what happens when the desktop isn't reachable: queue on the phone and retry; anything beyond the LAN, such as a relay, is out of scope unless it is end-to-end encrypted and needs no account.
- Pairing: a QR code on the desktop with a one-time secret and the desktop key. Devices keep each other's keys; revoking from either side.
- Transport: TLS with pinned self-signed certificates or a Noise handshake. Large uploads (an hour of Opus is ~14 MB) with resume.
- Tauri mobile's maturity for background recording and uploads on Android and iOS, versus native apps.
Prototype the riskiest part (pairing + an encrypted upload between a laptop and an Android phone) and record a decision (backlog decision create). Update AGENTS.md's local-first rule with what the decision allows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Prototype: a phone pairs with the desktop by QR code and uploads a recording over an encrypted connection
- [ ] #2 Decision recorded: discovery, pairing, encryption, offline behaviour, and Tauri mobile vs native
<!-- AC:END -->
