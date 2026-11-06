"""Screenshots for the README (light and dark), from demo data only.

Usage: python3 scripts/readme-screens.py <app binary> <data dir> <output dir>

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a temp folder holding only `engine/` and
`models/`. Imports the read speech in src-tauri/tests/fixtures (FLEURS, CC-BY-4.0) under made-up
names. The live screen plays a fixture into the system audio with the microphone muted from
the start, so nothing from the room gets in; the script fails if a mic caption shows up.
Writes <screen>-<theme>.png at 1600 px wide (needs ImageMagick and ffmpeg).
"""
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
OUT.mkdir(parents=True, exist_ok=True)
FIX = Path(__file__).parent.parent / "src-tauri/tests/fixtures"
tmp = Path(tempfile.mkdtemp())
demo = tmp / "Project review.wav"
subprocess.run(["ffmpeg", "-loglevel", "error", "-i", FIX / "en-two-speakers.wav", "-i", FIX / "nl-fleurs-1784.wav",
                "-filter_complex", "[0:a]apad=pad_dur=1[a];[a][1:a]concat=n=2:v=0:a=1", demo], check=True)


def done(s, n):
    end = time.time() + 300
    while time.time() < end:
        ms = s.invoke("list_meetings")
        if len(ms) >= n and all(m["status"] == "done" for m in ms):
            return ms
        time.sleep(1)
    raise SystemExit("imports did not finish")


def shoot(s, name):
    for theme in ("light", "dark"):
        s.js(f"document.documentElement.classList.toggle('dark', {str(theme == 'dark').lower()})")
        time.sleep(0.5)
        png = OUT / f"{name}-{theme}.png"
        s.screenshot(png)
        subprocess.run(["magick", png, "-resize", "1600x", "-strip", "-define", "png:compression-level=9", png], check=True)


def nav(s, name):
    s.js(f"[...document.querySelectorAll('nav button')].find(b => b.textContent.trim() == '{name}').click()")
    time.sleep(0.6)


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
s.invoke("import_audio", {"paths": [str(FIX / "en-fleurs-1652.wav"), str(FIX / "nl-fleurs-1784.wav")]})
done(s, 2)
s.invoke("import_audio", {"paths": [str(demo)]})
meetings = done(s, 3)
main = max(meetings, key=lambda m: m["duration_ms"])["id"]
others = [m["id"] for m in meetings if m["id"] != main]
for mid, title in zip(others, ("Team check-in", "Weekoverleg")):
    s.invoke("rename_meeting", {"id": mid, "title": title})
s.invoke("rename_meeting", {"id": main, "title": "Project review"})
for v, name in zip(s.invoke("meeting_voices", {"meetingId": main}), ("Anna", "Ben", "Carla")):
    s.invoke("name_speaker", {"meetingId": main, "label": v["label"], "name": name})
con = sqlite3.connect(DATA / "manent.db")
for at_ms, label in ((9_000, "Check the figures"), (41_000, "")):
    con.execute("INSERT INTO marks (meeting_id, at_ms, label, created_at) VALUES (?, ?, ?, 0)", (main, at_ms, label))
con.commit()
con.close()
s.close()

# A fresh start, so the screens read the data above.
s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')")
time.sleep(1)
shoot(s, "library")
s.js(f"document.querySelector('[data-meeting-id=\"{main}\"] [data-testid=open-meeting]').click()")
s.wait_for("return document.querySelector('[data-testid=waveform]')", 30)
s.wait_for("return document.querySelector('[data-testid=mark]')", 10)
shoot(s, "meeting")
nav(s, "Speakers")
shoot(s, "speakers")
s.js("document.querySelector('[data-testid=record-nav]').click()")
s.wait_for("return document.querySelector('[data-testid=start-recording]')", 5)
time.sleep(1)
shoot(s, "record")

player = subprocess.Popen(["paplay", "--volume=20000", str(FIX / "en-two-speakers.wav")])
s.js("document.querySelector('[data-testid=start-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=mute-toggle]')", 10)
s.js("document.querySelector('[data-testid=source-chip] [data-testid=mute-toggle]').click()")
s.js("const i = document.querySelector('[data-testid=meeting-title-input]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'Project review');"
     "i.dispatchEvent(new Event('input', {bubbles: true})); i.focus(); i.blur();")
# Enough text to show what a live transcript looks like (captions take a few seconds).
s.wait_for("const b = [...document.querySelectorAll('[data-testid=caption-block]')];"
           "return b.map(e => e.textContent).join('').length > 170", 34)
s.js("document.querySelector('[data-testid=mark-moment]').click()")
time.sleep(0.5)
if s.js("return document.querySelector('[data-testid=caption-block][data-kind=mic]')"):
    player.terminate()
    s.close()
    raise SystemExit("a mic caption showed up: no live screenshot from room audio")
shoot(s, "live")
player.terminate()
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)
nav(s, "Settings")
# The top of the page: further down the meetings folder shows a path of this machine.
s.js("document.querySelector('main').scrollTop = 0")
time.sleep(0.5)
shoot(s, "settings")
s.close()
