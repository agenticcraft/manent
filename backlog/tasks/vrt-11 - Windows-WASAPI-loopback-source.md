---
id: VRT-11
title: Windows WASAPI loopback source
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - audio
  - windows
milestone: m-1
dependencies:
  - VRT-4
documentation:
  - doc-2
priority: medium
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement AudioSource for Windows: mic via cpal WASAPI and system audio via WASAPI loopback. See doc-2 Technical architecture §1.2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Mic and system audio recorded concurrently on Windows 10/11
- [ ] #2 Device enumeration shows output devices as loopback sources
<!-- AC:END -->
