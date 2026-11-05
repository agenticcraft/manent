---
id: VRT-53
title: 'Spike: dictation into any text field with a shortcut'
status: To Do
assignee: []
created_date: '2026-11-04T21:43:00Z'
labels:
  - spike
milestone: m-4
dependencies:
  - VRT-40
priority: medium
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Requested (2026-11-04): use Manent for voice input outside meetings. Press a global shortcut, speak, press it again, and the text appears where the cursor is (a text field, a document, a chat). Builds on the global shortcuts portal (VRT-40, decision-10) and the local engine (parakeet, NL/EN); nothing leaves the computer. The open question is getting the text into another app on GNOME Wayland, where apps cannot type into each other: candidates are the RemoteDesktop portal (keyboard emulation after the user consents once), an IBus input-method engine (GNOME's own text input path), and the clipboard plus a paste, each with its own consent, reliability and latency. Also: mic only (no system audio, no speakers, no meeting created), and how fast a short clip transcribes with the 0.6B model (a few seconds of speech).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each way of inserting text into another app on GNOME Wayland is tried (RemoteDesktop portal, IBus engine, clipboard + paste): works in a browser field, LibreOffice and a terminal or not, what the user must allow, special characters (é, ë, quotes) and latency; findings in the implementation notes
- [ ] #2 Measured: time from the second key press to inserted text for 5 s and 30 s of speech, Dutch and English, on the test machine
- [ ] #3 A recommendation (with a Backlog decision if it adds a dependency) and follow-up tasks for building it, or a documented reason not to
<!-- AC:END -->
