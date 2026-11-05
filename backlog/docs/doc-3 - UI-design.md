---
id: doc-3
title: UI design
type: specification
created_date: 2026-10-29 12:18
updated_date: '2026-11-04T21:44:00Z'
---
## UI design: clean and calm

**Design principles:** calm, document-like and quiet. The transcript is the hero, and controls step back.
- **Typography:** Inter for the UI, a readable size (15–16 px) with generous line height for transcript text, and tabular numbers for timers.
- **Color:** neutral greys (zinc) with one accent color. The only strong signal is a soft red pulse while recording. Each named person has a muted pastel color (a filled chip); an unnamed "Speaker N" is an outlined chip with a colored dot, so it never looks like a person who happens to have that color. "Me" is a neutral chip. The app has light and dark modes and follows the OS setting.
- **Layout:** a thin left sidebar (Library, Speakers, Settings), with plenty of whitespace and no heavy borders. Sections are separated by subtle 1 px dividers or by spacing.
- **Motion:** minimal. Live words fade in gently, and the view auto-scrolls unless the user scrolls up, in which case a "Jump to live" pill appears.
- **Icon (VRT-43):** an "M" drawn as one continuous line, which is also a sound wave with two peaks, in white on a rounded tile in the accent colour (indigo `#4f39f6`). Chosen over a "waveform into text lines" mark and a speech bubble: it is the calmest, and it stays readable at 16 px. The tray uses the same tile; while recording, a red dot with a white ring sits at the bottom right. The source SVGs are in `src-tauri/icons/source/`; the other sizes come from `npx tauri icon`.
- **One column (VRT-42):** every screen uses the same content column, `max-w-3xl`, centred in the window. The page header's actions line up with the content below them. Transcript blocks reach into the gutter (`-mx-3`), so their text lines up with the title and the player while the highlight still has room.
- **Buttons:** one size per context. Settings and page headers use `sm`. The single main action of a screen is filled: Stop while recording, Download for the engine. "New recording" uses the accent. Secondary actions are outline buttons. Destructive actions start quiet (muted text, red on hover) and ask for confirmation.
- **Keyboard:** Space plays and pauses in the meeting view, unless a text field or button has the focus. Esc and a press outside close every popover and menu. Every control shows a visible focus outline (a base rule; components with their own ring keep it).
- **Untitled meetings** are shown by when they were held, e.g. "Sunday evening meeting", in slightly muted text, until the user gives a title. Nothing is stored, and the Live screen shows it as the title placeholder. Exports and `transcript.md` (Rust) still say "Untitled meeting".

**Screens:**
1. **Library:** a list of meetings showing title, date, duration and speaker avatars, plus "New recording" and "Import audio" buttons (recordings can also be dropped on the window). Imports and transcriptions in progress are listed above the meetings with their progress. The search box filters titles and finds text in every transcript; matches are grouped per meeting with its date and length, and a match opens the meeting at that moment.
2. **Live recording:**
   - The top bar shows the editable title, the timer, Mark (confirms "Marked 0:42" for a moment; also the mark-moment shortcut, VRT-52), and Pause and Stop buttons. Under it a source chip for each source with a mini level meter and a mute toggle, and on the right the Call / In the room switch (VRT-47).
   - Record is always at the top of the sidebar: idle it starts a recording, while recording it is the pulsing "Recording" that returns here (VRT-49).
   - The body is the transcript as speaker blocks: a colored name chip, a timestamp, then the text. The current speaker's block is highlighted, and partial text appears in a lighter grey until it is final.
   - Clicking a speaker chip opens a popover to name the speaker or merge them into another.
3. **Meeting detail:** under the title, who took part with their share of the talking (`<1%` for a word or two); clicking a name cues their first turn. Turns of up to 60 characters sit on one line with their speaker chip, so a quick exchange doesn't read as a column of headers. The same transcript view with a waveform player that stays at the bottom. Marks sit between the blocks (bookmark, time, label: click the time to play, the label to edit, × on hover to delete) and as small accent ticks above the waveform. Clicking a word (or a block's time) plays from there; the word and block being played are highlighted and the view follows them. It offers "Re-transcribe" (the same model, over the whole recording again) and Export (Markdown, TXT, SRT, JSON; VRT-21).
4. **Speakers:** known voices with name, color, number of meetings, and Rename, Merge and Delete actions.
5. **Settings:**
   - **Sources:** choose the mic and the output device to capture, test meters, and an "auto-detect default device" option.
   - **Transcription:** engine and model status, download and model licences (one model for Dutch and English), and how often speaker labels are updated while recording (the rolling pass, VRT-27). No language setting: the model detects the language itself and ignores a requested one (VRT-32, rechecked 2026-11-01).
   - **Shortcuts:** "Start or stop recording" and "Mark this moment" with their keys (Ctrl+Alt+R, Ctrl+Alt+M asked for); off until "Turn on", which lets the desktop ask once; then "Change keys…" opens the desktop's settings and "Turn off" (VRT-40, decision-10).
   - **Storage:** location and the format to keep.
