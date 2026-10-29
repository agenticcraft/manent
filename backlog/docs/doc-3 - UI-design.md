---
id: doc-3
title: UI design
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-10-29T17:05:00Z'
---
## UI design: clean and calm

**Design principles:** calm, document-like and quiet. The transcript is the hero, and controls step back.
- **Typography:** Inter for the UI, a readable size (15–16 px) with generous line height for transcript text, and tabular numbers for timers.
- **Color:** neutral greys (zinc) with one accent color. The only strong signal is a soft red pulse while recording. Each speaker gets a muted pastel color. The app has light and dark modes and follows the OS setting.
- **Layout:** a thin left sidebar (Library, Speakers, Settings), with plenty of whitespace and no heavy borders. Sections are separated by subtle 1 px dividers or by spacing.
- **Motion:** minimal. Live words fade in gently, and the view auto-scrolls unless the user scrolls up, in which case a "Jump to live" pill appears.

**Screens:**
1. **Library:** a searchable list of meetings showing title, date, duration and speaker avatars, plus "New recording" and "Import audio" buttons.
2. **Live recording:**
   - The top bar shows the editable title, the timer, a source chip for each source with a mini level meter and a mute toggle, and Pause and Stop buttons.
   - The body is the transcript as speaker blocks: a colored name chip, a timestamp, then the text. The current speaker's block is highlighted, and partial text appears in a lighter grey until it is final.
   - Clicking a speaker chip opens a popover to name the speaker or merge them into another.
3. **Meeting detail:** the same transcript view with a waveform player at the bottom. Clicking a word jumps playback to that point. It offers Export (Markdown, TXT, SRT, JSON) and "Re-transcribe with the high-accuracy model".
4. **Speakers:** known voices with name, color, number of meetings, and Rename, Merge and Delete actions.
5. **Settings:**
   - **Sources:** choose the mic and the output device to capture, test meters, and an "auto-detect default device" option.
   - **Transcription:** language (Auto, Nederlands, English), live and offline model per language, and the diarization latency setting. The live screen lets the user override the language for one meeting.
   - **Shortcuts:** the global hotkey for start/stop and whether the app starts minimized to the tray.
   - **Storage:** location and the format to keep.
