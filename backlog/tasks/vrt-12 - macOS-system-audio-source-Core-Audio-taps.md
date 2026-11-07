---
id: VRT-12
title: macOS system audio source (Core Audio taps)
status: In Progress
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-07T14:14:00Z'
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
- [x] #1 Mic and system audio recorded concurrently on macOS 14.6+ (cpal CoreAudio loopback)
- [ ] #2 Permission prompts handled with a clear UI message when denied
- [ ] #3 Older macOS (before 14.6): system audio clearly marked as unavailable, mic recording still works
- [x] #4 The pinned macOS arm64 engine downloads, starts and transcribes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Build and test on a Mac (macOS 26.6, arm64): toolchain, cargo test, debug build.
2. macOS bundle config in src-tauri/tauri.macos.conf.json (merged on macOS only): Info.plist with NSMicrophoneUsageDescription and NSAudioCaptureUsageDescription, entitlements (audio-input; the helper may dlopen the downloaded libparakeet), the engine helper in Contents/MacOS, ad-hoc signing with hardened runtime for local tests.
3. Test as an .app opened from Finder (TCC asks for Manent, not the terminal): mic + system audio concurrently.
4. Engine: download the pinned arm64 Metal build, check its dependencies (otool -L), transcribe.
5. Then: empty loopback track detected and explained in the UI, older macOS marked unavailable, headsets (in+out devices) as mic and loopback, follow default output changes.

6. (in scope, agreed 2026-11-07) An empty track never breaks transcription: final_pass skips tracks with less than 1 s of audio (error only when no track has audio); rolling_pass leaves out tracks more than 10 s behind the others, so a silent source no longer stops every rolling pass.

7. No-input notice (AC #2): LevelEvent gets no_input (nothing arrived from a source 3 s after it started; digital silence still counts as input, so an idle output is not flagged), from the recorder and the test monitor. The record page, Settings > Sources and the Live chips show a notice; on macOS with a button to System Settings > Privacy & Security (Privacy_AudioCapture / Privacy_Microphone) through a new open_privacy_settings command.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Mac test (macOS 26.6, arm64, debug binary started from a terminal): the mic records fine; the system track (BuiltInSpeakerDevice) opens without error but gets 0 samples (42-byte FLAC, 0.0 s). cpal 0.18.2 makes a per-device process tap (CATapDescription initWithProcesses:andDeviceUID:withStream:, exclusive, empty process list) plus an aggregate device. Likely cause: no NSAudioCaptureUsageDescription and no .app bundle, so TCC never asks for Manent. The UI did not flag the empty track. Transcription failed only because the engine was not downloaded yet in that data folder.

macOS bundle config (src-tauri/tauri.macos.conf.json, merged by Tauri on macOS only, so the Linux RPM config is untouched): src-tauri/Info.plist adds NSMicrophoneUsageDescription and NSAudioCaptureUsageDescription (Tauri merges it into the bundle and, in tauri dev, embeds it in the binary). Entitlements: the app gets com.apple.security.device.audio-input (src-tauri/macos/Manent.entitlements); the engine helper gets com.apple.security.cs.disable-library-validation (macos/manent-engine.entitlements), the app keeps library validation. Tauri signs the app but not files it copies (bundle.macOS.files puts manent-engine in Contents/MacOS), so build.beforeBundleCommand runs scripts/macos-sign-engine.sh (hardened runtime, $APPLE_SIGNING_IDENTITY or ad-hoc). Verified: npm run tauri build gives Manent.app with both binaries adhoc,runtime and their own entitlements; codesign --verify --deep --strict passes. Bundling uses target/release, so only release builds bundle. Still open: LSMinimumSystemVersion is Tauri's default 10.13; under the hardened runtime DYLD_LIBRARY_PATH (engine/helper.rs) is ignored, check when the engine runs.

Verified on macOS 26.6 (M1 Pro) with Manent.app (ad-hoc signed, hardened runtime, opened through Finder/open): meeting 3 recorded the mic (16.0 s) and system audio (BuiltInSpeakerDevice, 15.9 s, mean -19 dB, YouTube) concurrently; the final pass transcribed both (mic in Dutch, system in English; the YouTube bleed on the mic was removed). The pinned arm64 engine (latest-metal-darwin-arm64) downloaded, started on Metal (MTL0, 'set_concurrency(2): the compute device is not the CPU; keeping 1 backend') and loads under the hardened runtime: libparakeet.dylib needs no library next to it, so DYLD_LIBRARY_PATH being ignored does not matter. Meetings 1 and 2 (before system-audio consent was given) got 0 loopback samples without any error: a missing/pending consent is silent, so AC #2 needs detection of an empty loopback track. Found a cross-platform bug with an empty track: final_pass (rolling.rs) sends every track to the engine, the helper refuses to diarize no audio ('diarize: no audio', parakeet.rs:221) and the whole job fails, the good mic track included; rolling_pass waits for the shortest track (rolling.rs:271), so an empty track also stops every rolling pass.

Implemented plan steps 6-7. Empty track: final_pass skips tracks under 1 s (MIN_TRACK), errors 'nothing was recorded' when none has audio (the meeting keeps its transcript); rolling_pass leaves out tracks more than 10 s behind the newest (STALLED) and windows under 1 s. Tests with a one-word fake engine that refuses no audio like the helper (fail without the fix). No input: levels::InputWatch (no sample at all for 3 s; digital silence counts as input) sets LevelEvent.no_input in the recorder and the test monitor. NoInputNotice (record page, Settings > Sources, Live) explains it; on macOS a button opens System Settings at Privacy_AudioCapture / Privacy_Microphone (anchors found in SecurityPrivacyExtension.appex on macOS 26.6) via the new open_privacy_settings command; the URLs live in audio/macos.rs (privacy_settings_url, None on Linux/Windows); AppInfo.privacy_settings tells the UI. cargo fmt/clippy/test (143 passed) and npm run check pass; Manent.app rebuilt. Not yet: a hands-on check of the notice on the Mac with consent revoked; smoke scripts can't run on macOS (no tauri-driver) and the Linux scripts are not extended yet.

Mac test of the no-input notice (2026-11-07): with AudioCapture consent reset, the record page and the Live screen showed 'No system audio is coming in' with Open System Settings (screenshots by the user); meeting 8 with both consents granted recorded and transcribed mic (NL) and system (EN). Every rebuild is ad-hoc signed with a new cdhash, so macOS treats it as a new app: earlier grants (same name in System Settings) no longer apply and capture is silent without a prompt; tccutil reset Microphone/AudioCapture ai.agenticcraft.manent brings the prompts back. A stable signing identity would keep grants across builds (none on this Mac: 'security find-identity' finds 0). Without consent macOS delivers a constant value around -91 dBFS (mic in meetings 4-7, system in meeting 4), not 'no samples' and not zeros; an idle output delivers exact zeros. So the notice misses a mic without consent (meetings 6/7: mic flat, no notice). Meetings 6/7 had nothing to transcribe (helper run by hand: no words, no speakers, no error). The helper aborts at exit: GGML_ASSERT([rsets->data count] == 0) in ggml-metal-device.m:618 during exit() (C++ static destructors of the Metal backend).
<!-- SECTION:NOTES:END -->
