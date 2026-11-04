"""Screenshots of every screen, light and dark, at the default and the minimum window size
(VRT-42 review rounds). Needs audio hardware for the live screen.

Usage: python3 scripts/screens.py <app binary> <data dir> <screenshot dir>

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a folder with transcribed meetings
(some with named people). Writes <screen>-<size>-<theme>.png. A short recording is made for
the live screen and deleted again.
"""
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
OUT.mkdir(parents=True, exist_ok=True)
SIZES = {"default": (1200, 800), "min": (800, 560)}

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)


def size(w, h):
    s._wd("POST", f"{s.base}/window/rect", {"width": w, "height": h})
    time.sleep(0.4)


def nav(name):
    s.js(f"[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('{name}')).click()")
    time.sleep(0.6)


def shoot(name):
    for size_name, (w, h) in SIZES.items():
        size(w, h)
        for theme in ["light", "dark"]:
            s.js(f"document.documentElement.classList.toggle('dark', {str(theme == 'dark').lower()})")
            time.sleep(0.3)
            s.screenshot(OUT / f"{name}-{size_name}-{theme}.png")
    size(*SIZES["default"])


nav("Library")
shoot("library")
s.js("const i = document.querySelector('input[type=search], [data-testid=search]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'wetsvoorstel');"
     "i.dispatchEvent(new Event('input', {bubbles: true}))")
time.sleep(1.2)
shoot("library-search")
s.js("const i = document.querySelector('input[type=search], [data-testid=search]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, '');"
     "i.dispatchEvent(new Event('input', {bubbles: true}))")
time.sleep(0.6)

# A long meeting with named people.
mid = max(s.invoke("list_meetings"), key=lambda m: m["duration_ms"])["id"]
s.js(f"document.querySelector(\"[data-testid=meeting-row][data-meeting-id='{mid}'] [data-testid=open-meeting]\").click()")
s.wait_for("return document.querySelectorAll('[data-testid=word]').length", 20)
s.wait_for("return document.querySelector('[data-testid=waveform]')", 40)
shoot("meeting")
s.js("document.querySelector('main').scrollTop = 1500")
shoot("meeting-scrolled")
s.js("document.querySelector('main').scrollTop = 0")
chip = s.js("const c = document.querySelector('[data-testid=speaker-chip]'); if (c) c.click(); return !!c")
time.sleep(0.5)
if chip:
    shoot("meeting-speaker-popover")
    s.js("document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}))")
    s.js("document.body.click()")
s.js("document.querySelector('[data-testid=export]').click()")
time.sleep(0.4)
shoot("meeting-export-menu")
s.js("document.querySelector('[data-testid=export]').click()")
s.js("document.querySelector('[data-testid=back]').click()")
time.sleep(0.5)

nav("Speakers")
shoot("speakers")
nav("Settings")
shoot("settings")
for i, y in enumerate([700, 1400, 2100]):
    s.js(f"document.querySelector('main').scrollTop = {y}")
    shoot(f"settings-{i + 2}")
s.js("document.querySelector('main').scrollTop = 0")

# Live: a short recording, then deleted again.
nav("Library")
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
time.sleep(3)
shoot("live")
live = s.invoke("current_recording")["meeting_id"]
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 30)  # Stop opens the recording
s.js("document.querySelector('[data-testid=back]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 10)
time.sleep(2)
for _ in range(60):
    if not any(j["meeting_id"] == live and j["stage"] not in ("done", "failed") for j in s.invoke("list_jobs")):
        break
    time.sleep(1)
s.invoke("delete_meeting", {"id": live})
s.close()
print(f"screenshots in {OUT}")
