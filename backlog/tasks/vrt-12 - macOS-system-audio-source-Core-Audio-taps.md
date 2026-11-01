---
id: VRT-12
title: macOS system audio source (Core Audio taps)
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T09:53:00Z'
labels:
  - audio
  - macos
milestone: m-1
dependencies:
  - VRT-4
documentation:
  - doc-2
priority: medium
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Record system audio on macOS. cpal 0.18 supports CoreAudio loopback on macOS 14.6 and newer: an input stream on an output device, for which cpal creates a process tap plus an aggregate device. So the same CpalSource as on Linux should work, without a shim of our own (doc-2 §1.2). Verify it on a Mac, together with the system-audio capture permission (Info.plist usage string, the prompt, and a clear message when it is denied). macOS before 14.6 is not supported for system audio (mic only); ScreenCaptureKit would be the route if that is ever needed. The engine for macOS arm64 is already pinned (engine/assets.rs, untested).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Mic and system audio recorded concurrently on macOS 14.6+ (cpal CoreAudio loopback)
- [ ] #2 Permission prompts handled with a clear UI message when denied
- [ ] #3 Older macOS (before 14.6): system audio clearly marked as unavailable, mic recording still works
- [ ] #4 The pinned macOS arm64 engine downloads, starts and transcribes
<!-- AC:END -->
