---
id: VRT-12
title: macOS system audio source (Core Audio taps)
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
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
Implement AudioSource for macOS: mic via cpal CoreAudio and system audio via Core Audio process taps (macOS 14.4+), ScreenCaptureKit fallback, via an objc2 shim. Handle permission prompts and Info.plist entries. See doc-2 Technical architecture §1.2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Mic and system audio recorded concurrently on macOS 14.4+
- [ ] #2 Permission prompts handled with a clear UI message when denied
- [ ] #3 Fallback path documented or implemented for older macOS
<!-- AC:END -->
