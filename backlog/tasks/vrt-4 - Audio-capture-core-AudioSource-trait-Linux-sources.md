---
id: VRT-4
title: 'Audio capture core: AudioSource trait + Linux sources'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T21:32:00Z'
labels:
  - audio
  - backend
  - linux
milestone: m-1
dependencies:
  - VRT-1
  - VRT-3
documentation:
  - doc-2
priority: high
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the audio module: AudioSource trait, device enumeration (SourceInfo {id, name, kind: Mic|Loopback}), Linux implementations for mic and PipeWire monitor, and lock-free ring buffers (rtrb) so realtime callbacks never block. See doc-2 Technical architecture §1.2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tauri command lists available mic and loopback sources
- [x] #2 Starting/stopping a source delivers samples through a ring buffer without blocking the audio callback
- [x] #3 Multiple sources can run concurrently
- [x] #4 Unit tests cover ring buffer and enumeration mapping
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Deps: cpal 0.18 (feature pipewire on Linux only), rtrb, thiserror.
2. audio/ring.rs: SampleRing (rtrb) split into a realtime-safe writer (never blocks, counts dropped samples) and a reader; unit tests for order, wrap-around, overflow/drop counting.
3. audio/sources.rs: SourceKind (mic|loopback), SourceInfo {id, name, kind, is_default}; pure classify() from device facts (input/output support, default ids, virtual default_* devices skipped); unit tests.
4. Platform host per cfg module: Linux = cpal PipeWire host (fallback default host), Windows/macOS = default host. Enumerate via cpal; loopback = input stream on an output device (validated in VRT-1).
5. audio/capture.rs: AudioSource trait (info, format, start, stop) + CpalSource: f32 input stream at the device's default rate/channels; callback only pushes into the ring.
6. Tauri command list_sources + ipc.ts types.
7. Hardware integration test (#[ignore], run locally): default mic + default loopback concurrently for 2 s -> samples arrive, zero drops, stop works.
8. Verify: fmt, clippy, unit + hardware tests, call list_sources from the real app via ui-smoke.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation (src-tauri/src/audio): ring.rs (rtrb SampleRing: realtime writer never blocks, counts dropped samples; reader drains), sources.rs (SourceKind, SourceInfo, pure classify()), capture.rs (AudioSource trait, CpalSource, list_sources), linux.rs/windows.rs/macos.rs selected as 'platform' via #[path]. cpal's default host is already the PipeWire host on Linux with the pipewire feature (target-specific dependency).
Finding: on PipeWire, default_input_device()/default_output_device() return the virtual aliases (pipewire:input_default / output_default), whose ids never match a real device, so the first version flagged no default and the test picked the wrong mic (Mic1 instead of the default Mic2). Fix: when the OS default is a virtual alias it is listed as 'System default microphone' / 'System default output' (is_default) and follows the OS default; other aliases (default_sink) are skipped. Useful for VRT-9 ('follows OS default changes').
Finding: PipeWire reports 'Device disconnected' to the error callback while a stream is intentionally dropped (confirmed with markers around stop). CpalSource sets a stopping flag first and the error callback ignores errors during teardown.
Verification: cargo fmt/clippy clean (0 warnings; temporary dead_code allows on capture/ring until VRT-5, noted there); 12 unit tests pass; hardware test (cargo test -- --ignored audio_hardware) records mic + loopback concurrently for 2 s: 1.92-1.98 s delivered each, 0 dropped, no samples after stop, both with system-default aliases and with real devices (VERITAS_AUDIO_TEST_REAL=1); ui-smoke calls list_sources inside the real app: 5 sources (2 system defaults, 2 mics, speaker monitor).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Audio capture core in place: cpal-based AudioSource trait with a CpalSource that opens mic or loopback sources (loopback = input stream on an output device) and pushes f32 samples from the realtime callback into a lock-free rtrb ring (never blocks, counts drops). list_sources maps OS devices to Mic/Loopback sources, lists PipeWire's default aliases as 'System default microphone/output' so recording follows the OS default, and is exposed as a Tauri command with matching ipc.ts types. Verified with 12 unit tests, a hardware test capturing mic + loopback concurrently (0 drops, clean stop, no spurious teardown errors) and a call from the running app.
<!-- SECTION:FINAL_SUMMARY:END -->
