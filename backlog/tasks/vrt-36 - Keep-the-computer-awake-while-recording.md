---
id: VRT-36
title: Keep the computer awake while recording
status: To Do
assignee: []
created_date: '2026-11-01T11:15:00Z'
labels:
  - recording
  - platform
milestone: m-2
dependencies: []
priority: high
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in VRT-35: during a 25-minute live recording the laptop went into idle suspend (GNOME, no lid event) for 7 minutes. Capture and playback froze and resumed afterwards, so that part of the meeting was lost. A meeting recorder should stop idle sleep while it records and release the block when the recording stops or the app quits. Per platform behind a small trait/cfg module: Linux logind inhibitor (org.freedesktop.login1 Inhibit, what=idle:sleep, mode=block; needs a D-Bus client such as zbus, so a backlog decision first), macOS IOPMAssertionCreateWithName (PreventUserIdleSystemSleep), Windows SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED). Closing the lid still suspends (the user's choice); after a resume, the recording continues and the gap is noted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Idle suspend does not happen while a recording runs (verified on Linux with a short idle-suspend timeout)
- [ ] #2 The block is released on stop, on app exit and if the app crashes (inhibitor tied to the process)
- [ ] #3 After a lid-close suspend the recording continues and the meeting shows where audio is missing
- [ ] #4 Decision recorded for the D-Bus dependency
<!-- AC:END -->
