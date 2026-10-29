---
id: VRT-9
title: 'Settings: audio sources'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T22:30:00Z'
labels:
  - frontend
  - audio
milestone: m-1
dependencies:
  - VRT-4
  - VRT-7
documentation:
  - doc-3
priority: medium
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Settings > Sources: pick microphone and output device to capture, test meters, and 'auto-detect default device'. Persist choices. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Device pickers list mic and loopback sources
- [x] #2 Test meter shows live level for the selected device
- [x] #3 Auto-detect default device follows OS default changes
- [x] #4 Choices persisted in settings table
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. audio/monitor.rs: Monitor = capture thread like the recorder but levels only (no files); LevelEvent callback at ~20 Hz; stops on drop. Unit-testable core shared with the recorder (level per ring).
2. Commands: get_recording_sources (available sources + the resolved mic/system choice), set_recording_sources (mic id, system id or null = don't record) -> settings 'recording.sources'; start_source_monitor / stop_source_monitor emitting 'audio:monitor-level'; start_recording stops a running monitor.
3. Settings > Sources UI: per kind a native select (System default first: follows the OS), a live test meter for the selected device, 'Don't record system audio' option; saved on change; monitor runs while Settings is open.
4. Verify: Rust tests (settings round trip/resolution); smoke check on the real app with a temp data dir: pickers list mics/loopbacks, meters move for the selected devices, choice persisted in the DB and used by the next recording; 'System default' follows OS default changes: with the monitor on the default mic, switch the OS default source with pactl, confirm the stream moves (pactl source-outputs), then restore the original default.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation: audio/monitor.rs Monitor (capture thread reporting levels only, stops on drop); commands get_recording_sources / set_recording_sources (settings 'recording.sources' = [mic, system?]; source_settings splits the resolved choice), start_source_monitor / stop_source_monitor ('audio:monitor-level'), start_recording stops a running monitor; src/components/SourcesSettings.tsx in Settings (selects with System default first and 'Don't record system audio', live test meters, saved on change). scripts/smoke-settings.py; AGENTS.md updated.
Findings and fixes during verification:
- The 'Digital Microphone' (Mic1) on the test machine is dead: recorded directly (outside the app) it outputs a constant -0.177 (3 distinct values in 2 s). The meter showed a healthy-looking -15 dB for it. levels::measure now computes RMS around the block mean, so a DC offset reads as silence (peak still shows it); tests that used constant blocks as 'signal' switched to AC square waves.
- Native <select> rendered light-on-light in dark mode (WebKitGTK draws native controls in the light theme): added color-scheme light/dark to the theme and styled the select fully (appearance-none + own chevron).
Verification: 40 Rust tests (new: monitor level per tick, DC offset -> silence, source_settings split); hardware monitor test 19 events/s, stops on drop; clippy 0; npm check clean. smoke-settings.py on the real app: pickers list 'System default microphone' + 2 mics and 'System default output' + speaker + 'Don't record'; mic test meter live; with the monitor on System default, switching the OS default source with pactl moved the app's stream Mic2 -> Mic1 (events kept flowing at 20 Hz), default restored to Mic2 afterwards and the meter was live again; choosing Mic1 + no system audio stored ['pipewire:...Mic1...'] in settings, shown again after an app restart and used by the next start_recording. Light/dark screenshots inspected after the select fix. smoke-live, smoke-library and ui-smoke still pass; OS default and real data dir untouched.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Settings > Sources lets the user pick the microphone and the system audio to record (or no system audio), with live test meters for the selected devices; 'System default' entries follow the OS default device, also mid-recording (verified by switching the OS default with pactl and watching the stream move). Choices are stored in the settings table and used by the next recording. Two issues found and fixed along the way: a DC-offset dead mic no longer shows a fake level, and native selects are readable in dark mode.
<!-- SECTION:FINAL_SUMMARY:END -->
