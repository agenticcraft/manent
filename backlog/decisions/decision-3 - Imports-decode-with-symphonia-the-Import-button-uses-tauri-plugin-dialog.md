---
id: decision-3
title: Imports decode with symphonia; the Import button uses tauri-plugin-dialog
date: '2026-10-31T20:46:00Z'
status: accepted
---
## Context

VRT-19 imports recordings made elsewhere: wav, mp3, m4a and mp4 (AAC audio, for example a Teams or Zoom recording). The app already reads FLAC (claxon), but it can't decode MP3 or AAC. Two other routes were ruled out:
- Calling a system ffmpeg is not possible: it isn't on every desktop.
- Asking the engine helper is not possible: libparakeet only reads WAV.

The "Import audio" button also needs a native file dialog. Webviews can't hand out file paths, and Tauri core has no dialog.

## Decision

- **Decoding:** `symphonia` (pure Rust, no system libraries) with the `mp3`, `aac`, `alac` and `isomp4` features on top of its defaults (wav/pcm, flac, ogg/vorbis, mkv). Opus is not supported by symphonia and is not offered.
- **Dialog:** `tauri-plugin-dialog` 2 for the Import button, allowed only `dialog:allow-open`.
- **Drag and drop:** dropping files on the window uses Tauri's built-in drag-and-drop event and needs no plugin.

## Consequences

- The supported formats are wav, mp3, m4a/mp4/aac, flac, ogg/vorbis and mkv/webm with those codecs. Anything else gets a clear "can't read this file" error.
- An imported file is decoded once into the meeting's own FLAC track. The meeting then plays and re-transcribes like a recording, and the original file is not needed afterwards.

