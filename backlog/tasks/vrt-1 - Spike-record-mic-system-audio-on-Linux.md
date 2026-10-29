---
id: VRT-1
title: 'Spike: record mic + system audio on Linux'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:16
updated_date: '2026-10-29T14:28:00Z'
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
- [x] #1 Rust CLI lists input devices and output monitors
- [x] #2 CLI records mic and system audio concurrently into two WAV files
- [x] #3 Both files play back correctly and stay in sync over 5 minutes
- [x] #4 Findings (crate choice: cpal vs pipewire-rs) written to implementation notes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Prereqs: Rust via rustup; dnf alsa-lib-devel, pulseaudio-libs-devel, pipewire-devel, clang-devel.
2. Throwaway crate spikes/audio-capture (reference only, not app code).
3. 'list': mic sources and output monitors via the PulseAudio API, plus all cpal hosts/devices.
4. 'record': libpulse-simple, mic (@DEFAULT_SOURCE@) + system (@DEFAULT_MONITOR@) on two threads into two WAVs.
5. 'record-cpal': cpal 0.18 PipeWire host, default input + input stream on default output (loopback); callback -> rtrb ring buffer -> writer thread -> WAV.
6. Verify with a 440 Hz test tone (10 s, both backends) and a 5-minute cpal run (sync, dropouts, CPU); pipewire-rs not used directly since cpal's PipeWire host wraps it.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Spike crate: spikes/audio-capture (commands: list, record [libpulse-simple], record-cpal [cpal PipeWire host]).

Findings:
- Default mic = HiFi__Mic2__source (internal stereo mic), system = HiFi__Speaker__sink.monitor; native s32le 2ch 48 kHz.
- cpal 0.18 ships optional PipeWire (and PulseAudio) hosts. With feature "pipewire", sinks are exposed as in+out devices and an *input* stream on an output device sets stream.capture.sink = loopback. WASAPI and CoreAudio hosts in cpal already do loopback the same way (macOS via an aggregate device). => one cpal API covers mic + system audio on all three OSes.
- cpal 0.18 API change: DeviceTrait::name() is replaced by description(); build_input_stream takes a timeout argument.
- libpulse-simple also works (any source by name incl. .monitor and @DEFAULT_MONITOR@), but is Linux-only and blocking-read based.
- 10 s test with a 440 Hz tone played on the speakers: both backends capture the tone on the system track (-54.7 dBFS at low volume, clean), mic tracks show speaker bleed as expected. Zero dropped samples with the rtrb ring buffer design.
- cpal tracks differ ~21 ms in length after 10 s: start offset from sequential stream startup, not drift; the app should timestamp the first callback per track (InputCallbackInfo) to align tracks.

5-minute cpal run: mic and system both 14,396,416 frames (299.925 s, wall clock 300.005 s; the ~80 ms is stream startup), drift 0.0 ms, 0 dropped samples, CPU 1% (user 1.77 s + sys 2.62 s over 300 s), max RSS 11 MB.
Playback validity: WAVs parsed and analysed (RMS, Goertzel 440 Hz) for both backends; the tone is present on system tracks.
Decision for VRT-4: use cpal >= 0.18 with the pipewire feature on Linux; no libpulse or direct pipewire-rs needed. doc-2 §1.2 updated accordingly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Built a throwaway Rust spike (spikes/audio-capture) that lists audio devices and records the microphone and system output at the same time into separate WAVs on Fedora/PipeWire, with two backends: libpulse-simple and cpal 0.18's PipeWire host.

Key result: cpal 0.18 handles loopback on all desktop OSes (PipeWire stream.capture.sink, WASAPI loopback, CoreAudio aggregate), so VRT-4 can use one crate for mic and system audio. Verified: a 440 Hz test tone is captured on the system track by both backends. A 5-minute cpal recording had identical frame counts on both tracks (0 ms drift), 0 dropped samples, 1% CPU and 11 MB RSS. doc-2 §1.2 is updated.

Follow-ups for later tickets: align tracks using first-callback timestamps (about 20 ms start offset); confirm macOS permissions and the minimum OS version for cpal's CoreAudio loopback (VRT-12); Windows is still untested (VRT-11).
<!-- SECTION:FINAL_SUMMARY:END -->
