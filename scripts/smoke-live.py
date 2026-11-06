"""Live recording smoke check through the real UI.

Usage: python3 scripts/smoke-live.py <app binary> <data dir> <screenshot dir> [<tone.wav>]

tauri-driver must run with MANENT_DATA_DIR=<data dir> (an empty temp folder). Needs audio
hardware. If a WAV is given it is played (quietly) so the system-audio meter has signal.
"""
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
TONE = sys.argv[4] if len(sys.argv) > 4 else None

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
player = subprocess.Popen(["paplay", "--volume=20000", TONE]) if TONE else None

# Start
s.js("document.querySelector('[data-testid=new-recording]').click()")
live = s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
t_start = time.time()
s.check(live == 2, f"Live screen with {live} source chips")
s.check(bool(s.js("return document.querySelector('[data-testid=recording-nav]')")), "sidebar shows 'Recording'")
indicator = s.js("return document.querySelector('[data-testid=recording-indicator]').getAttribute('aria-label')")
s.check(indicator == "Recording", f"recording indicator: {indicator}")

# Meters move
samples = []
for _ in range(10):
    samples.append(s.js("return [...document.querySelectorAll('[data-testid=level-meter]')].map(m => +m.getAttribute('aria-valuenow'))"))
    time.sleep(0.2)
mic = [v[0] for v in samples]
system = [v[1] for v in samples]
s.check(len(set(mic)) > 2 and max(mic) > 0, f"mic meter animates with room noise: {mic}")
if player:
    # A steady tone gives a steady level; check the meter responds when it stops.
    s.check(min(system) > 0, f"system meter shows the playing tone: {system}")
    player.terminate()
    player.wait()
    player = None
    time.sleep(0.8)
    after = s.js("return +document.querySelectorAll('[data-testid=level-meter]')[1].getAttribute('aria-valuenow')")
    s.check(after < min(system), f"system meter drops when the tone stops: {min(system)} -> {after}")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "live-light.png")
s.js("document.documentElement.classList.toggle('dark', matchMedia('(prefers-color-scheme: dark)').matches)")

# Title edited and saved on blur
set_value = ("const i = document.querySelector('[data-testid=meeting-title-input]');"
             "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
             "i.dispatchEvent(new Event('input', {bubbles: true})); i.focus(); i.blur();")
s.js(set_value, "Smoke test meeting")
time.sleep(0.5)
meeting_id = s.invoke("current_recording")["meeting_id"]
con = sqlite3.connect(DATA / "manent.db")
title = con.execute("SELECT title FROM meetings WHERE id = ?", (meeting_id,)).fetchone()[0]
con.close()
s.check(title == "Smoke test meeting", f"title persisted while recording: '{title}'")

# Call or in the room (VRT-47): a call on a fresh install; switching is stored on the meeting.
def kind():
    return s.js("return document.querySelector('[data-testid=meeting-kind]').dataset.inRoom")
def in_room_db(mid):
    con = sqlite3.connect(DATA / "manent.db")
    v = con.execute("SELECT in_room FROM meetings WHERE id = ?", (mid,)).fetchone()[0]
    con.close()
    return v
s.check(kind() == "false" and in_room_db(meeting_id) == 0, "a new install records a call")
s.js("document.querySelector('[data-testid=kind-room]').click()")
time.sleep(0.4)
s.check(kind() == "true" and in_room_db(meeting_id) == 1, "switched to 'In the room' while recording (stored)")
for dark in (True, False):
    s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
    time.sleep(0.4)
    s.screenshot(OUT / f"live-in-room-{'dark' if dark else 'light'}.png")
s.js("document.documentElement.classList.toggle('dark', matchMedia('(prefers-color-scheme: dark)').matches)")

# Mute the microphone for ~1.5 s
mic_mute = "[...document.querySelectorAll('[data-testid=source-chip]')][0].querySelector('[data-testid=mute-toggle]').click()"
s.js(mic_mute)
time.sleep(0.3)
muted = s.js("return document.querySelectorAll('[data-testid=source-chip]')[0].dataset.muted")
s.check(muted == "true", "mic chip shows muted")
s.js("document.documentElement.classList.add('dark')")
time.sleep(0.6)
s.screenshot(OUT / "live-dark-muted.png")
s.js("document.documentElement.classList.toggle('dark', matchMedia('(prefers-color-scheme: dark)').matches)")
time.sleep(1.2)
s.js(mic_mute)

# Pause stops the timer, resume continues
s.js("document.querySelector('[data-testid=pause-toggle]').click()")
time.sleep(0.6)
t1 = s.js("return document.querySelector('[data-testid=recording-timer]').textContent")
time.sleep(2.0)
t2 = s.js("return document.querySelector('[data-testid=recording-timer]').textContent")
paused_label = s.js("return document.querySelector('[data-testid=recording-indicator]').getAttribute('aria-label')")
s.check(t1 == t2 and paused_label == "Paused", f"paused: timer stays {t1} -> {t2}, indicator {paused_label}")
s.screenshot(OUT / "live-paused.png")
s.js("document.querySelector('[data-testid=pause-toggle]').click()")
time.sleep(1.5)
t3 = s.js("return document.querySelector('[data-testid=recording-timer]').textContent")
s.check(t3 != t2, f"resumed: timer moves {t2} -> {t3}")

# Stop -> Library with the meeting
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)  # Stop opens the recording
s.js("document.querySelector('[data-testid=back]').click()")
row = s.wait_for("return document.querySelector('[data-testid=meeting-row]')?.innerText", 10)
wall = time.time() - t_start
if player:
    player.terminate()
s.check(bool(row) and "Smoke test meeting" in row, f"meeting listed in Library: {row!r}")
s.check(not s.js("return document.querySelector('[data-testid=recording-nav]')"), "'Recording' gone from sidebar")
s.check(bool(s.js("return document.querySelector('[data-testid=record-nav]')")), "sidebar shows 'Record' when idle")
s.screenshot(OUT / "library-after-recording.png")

# Record from the sidebar, from another screen (VRT-49), and back to it from elsewhere.
s.js("[...document.querySelectorAll('nav button')].find(b => b.textContent.trim() == 'Settings').click()")
time.sleep(0.5)
s.js("document.querySelector('[data-testid=record-nav]').click()")
ready = s.wait_for("return document.querySelector('[data-testid=start-recording]')", 5)
time.sleep(0.8)
s.check(bool(ready) and s.invoke("current_recording") is None and not s.js("return document.querySelector('[data-testid=recording-nav]')"),
        "Record in the sidebar opens the record page from Settings, without recording")
# A quiet room reads 0 on the mic, so play something into the system audio for the meter.
fixture = Path(__file__).parent.parent / "src-tauri/tests/fixtures/en-fleurs-1652.wav"
test_sound = subprocess.Popen(["paplay", "--volume=20000", str(fixture)])
meter = s.wait_for("return [...document.querySelectorAll('[data-testid=level-meter]')].some(m => +m.getAttribute('aria-valuenow') > 0)", 8)
test_sound.terminate()
s.check(bool(meter), "the record page's test meters move")
s.check(kind() == "true", "the record page shows the last choice: 'In the room'")
s.js("document.querySelector('[data-testid=kind-call]').click()")
time.sleep(0.4)
s.check(kind() == "false" and s.invoke("get_in_room") is False, "switching on the record page is saved")
s.js("document.querySelector('[data-testid=kind-room]').click()")
time.sleep(0.4)
for dark in (True, False):
    s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
    time.sleep(0.4)
    s.screenshot(OUT / f"record-ready-{'dark' if dark else 'light'}.png")
s.js("[...document.querySelectorAll('nav ul button')].find(b => b.textContent.trim() == 'Library').click()")
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 5)
s.check(s.invoke("current_recording") is None, "leaving the record page records nothing")
s.js("document.querySelector('[data-testid=record-nav]').click()")
s.wait_for("return document.querySelector('[data-testid=start-recording]')", 5)
s.js("document.querySelector('[data-testid=start-recording]').click()")
chips = s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
s.check(chips == 2 and bool(s.js("return document.querySelector('[data-testid=recording-nav]')")),
        "Start recording starts it")
second = s.invoke("current_recording")
s.check(second["in_room"] and kind() == "true" and in_room_db(second["meeting_id"]) == 1,
        "the next recording starts 'In the room', as the last one")
s.js("[...document.querySelectorAll('nav button')].find(b => b.textContent.trim() == 'Speakers').click()")
time.sleep(0.5)
for dark in (True, False):
    s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
    time.sleep(0.4)
    s.screenshot(OUT / f"sidebar-recording-{'dark' if dark else 'light'}.png")
s.js("document.querySelector('[data-testid=recording-nav]').click()")
back = s.wait_for("return document.querySelector('[data-testid=stop]')", 5)
s.check(bool(back), "'Recording' in the sidebar goes back to the running recording")
time.sleep(1)
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)
s.js("document.querySelector('[data-testid=back]').click()")
s.wait_for("return document.querySelector('[data-testid=record-nav]')", 10)
for dark in (True, False):
    s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
    time.sleep(0.4)
    s.screenshot(OUT / f"sidebar-idle-{'dark' if dark else 'light'}.png")
s.close()

# Database and files
con = sqlite3.connect(DATA / "manent.db")
status, duration_ms, folder = con.execute("SELECT status, duration_ms, dir FROM meetings WHERE id = ?", (meeting_id,)).fetchone()
tracks = con.execute("SELECT source_kind, path, seconds FROM tracks WHERE meeting_id = ? ORDER BY id", (meeting_id,)).fetchall()
con.close()
expected = wall - 2.6  # one ~2.6 s pause
print(f"     meeting {meeting_id}: status {status}, {duration_ms} ms (wall {wall:.1f}s), dir {folder}")
s.check(status == "done" and abs(duration_ms / 1000 - expected) < 1.5, f"meeting finished: status {status}, {duration_ms / 1000:.1f}s vs ~{expected:.1f}s")
s.check(Path(folder).name == str(meeting_id) and Path(folder).parent == DATA / "meetings", "folder named after the meeting id")
s.check([t[0] for t in tracks] == ["mic", "loopback"] and all(Path(t[1]).is_file() for t in tracks), f"track rows + files: {tracks}")
if len(tracks) == 2:
    s.check(abs(tracks[0][2] - tracks[1][2]) < 0.1, f"tracks aligned: {tracks[0][2]:.3f}s vs {tracks[1][2]:.3f}s")
    pcm = subprocess.run(["ffmpeg", "-loglevel", "error", "-i", tracks[0][1], "-ac", "1", "-f", "s16le", "-"],
                         capture_output=True, check=True).stdout
    sr = 48000  # device rate; only used to express the run length in seconds
    zeros = longest = 0
    for i in range(0, len(pcm), 2):
        if pcm[i] == 0 and pcm[i + 1] == 0:
            zeros += 1
            longest = max(longest, zeros)
        else:
            zeros = 0
    s.check(longest / sr > 1.0, f"muted stretch is digital silence in mic.flac ({longest / sr:.2f}s of exact zeros)")

# After a restart the last choice still holds; switching back to a call is stored too.
earlier = s.failures
s = Session(APP)
s.failures = earlier + s.failures
s.wait_for("return document.querySelector('[data-testid=record-nav]')")
s.js("document.querySelector('[data-testid=record-nav]').click()")
s.wait_for("return document.querySelector('[data-testid=start-recording]')", 5)
s.js("document.querySelector('[data-testid=start-recording]').click()")
s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
third = s.invoke("current_recording")["meeting_id"]
s.check(kind() == "true", "after a restart the recording starts 'In the room'")
s.js("document.querySelector('[data-testid=kind-call]').click()")
time.sleep(0.4)
s.check(kind() == "false" and in_room_db(third) == 0, "switched back to 'Call' (stored)")
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)
s.close()
con = sqlite3.connect(DATA / "manent.db")
remembered = con.execute("SELECT value FROM settings WHERE key = 'recording.in_room'").fetchone()
con.close()
s.check(remembered is not None and remembered[0] == "false", f"the last choice is saved: {remembered}")

print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
