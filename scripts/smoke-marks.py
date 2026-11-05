"""Marks and shortcuts smoke check (VRT-52, VRT-40).

Usage: python3 scripts/smoke-marks.py <app binary> <data dir> <screenshot dir> [--bind] [--speech <wav>]

Records ~7 s, marks a moment with the Live screen's Mark button and one through mark_moment
(the path the mark-moment shortcut takes), stops, then labels and deletes marks on the
meeting page, checks the player's ticks and a restart. Settings > Shortcuts is checked too;
with --bind it is turned on and off, which needs ai.agenticcraft.manent.desktop installed
(the RPM, or a copy in ~/.local/share/applications) and the keys accepted once in GNOME's
dialog (decision-10). With --speech (the engine in the data folder; the WAV is played quietly
into the system audio) a second recording checks marks among real transcript blocks, live and
after the final pass, in transcript.md and in the Markdown, text and JSON exports.

tauri-driver must run with MANENT_DATA_DIR=<data dir> (an empty temp folder). Needs audio
hardware; no engine.
"""
import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
BIND = "--bind" in sys.argv[4:]
SPEECH = sys.argv[sys.argv.index("--speech") + 1] if "--speech" in sys.argv else None


def marks_db(mid):
    con = sqlite3.connect(DATA / "manent.db")
    rows = con.execute("SELECT at_ms, label FROM marks WHERE meeting_id = ? ORDER BY at_ms", (mid,)).fetchall()
    con.close()
    return rows


def themed(s, name):
    for dark in (True, False):
        s.js(f"document.documentElement.classList.toggle('dark', {str(dark).lower()})")
        time.sleep(0.4)
        s.screenshot(OUT / f"{name}-{'dark' if dark else 'light'}.png")


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=record-nav]')")
s.js("document.querySelector('[data-testid=record-nav]').click()")
s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
meeting_id = s.invoke("current_recording")["meeting_id"]
time.sleep(2.2)
s.js("document.querySelector('[data-testid=mark-moment]').click()")
shown = s.wait_for("const b = document.querySelector('[data-testid=mark-moment]');"
                   "return b.textContent.includes('Marked') && b.textContent", 3)
s.check(bool(shown) and "0:02" in shown, f"the Mark button confirms the moment: {shown!r}")
s.check(bool(s.wait_for("return document.querySelector('[data-testid=mark]')", 3)),
        "the mark shows on the Live screen at once, before any transcript")
themed(s, "live-marked")
time.sleep(2.0)
via_ipc = s.invoke("mark_moment")
first_ms = s.invoke("list_marks", {"meetingId": meeting_id})[0]["at_ms"]
s.check("at_ms" in via_ipc and via_ipc["at_ms"] >= first_ms + 2000, f"mark_moment (the shortcut's path) marks the time: {via_ipc}")
count = s.wait_for("return Number(document.querySelector('[data-testid=mark-moment]').dataset.marks) === 2", 3)
s.check(bool(count), "the Live screen knows both marks (also the one it didn't make)")
time.sleep(2.0)
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)
rows = marks_db(meeting_id)
s.check(len(rows) == 2 and 1800 <= rows[0][0] <= 3000, f"two marks stored: {rows}")
s.check(s.invoke("mark_moment").get("error", "").startswith("Not recording"), "no marks without a recording")

# The meeting page: marks in the transcript, ticks on the player.
n = s.wait_for("return document.querySelectorAll('[data-testid=mark]').length", 10)
s.check(n == 2, f"the meeting page shows {n} marks")
ticks = s.wait_for("return document.querySelectorAll('[data-testid=player-mark]').length", 60)
s.check(ticks == 2, f"the player shows {ticks} ticks")
s.js("document.querySelector('[data-testid=mark-label]').click()")
s.wait_for("return document.querySelector('[data-testid=mark-label-input]')", 3)
set_value = ("const i = document.querySelector('[data-testid=mark-label-input]');"
             "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
             "i.dispatchEvent(new Event('input', {bubbles: true}));"
             "i.dispatchEvent(new KeyboardEvent('keydown', {key: arguments[1], bubbles: true}));")
s.js(set_value, "Budget decision", "Escape")
time.sleep(0.5)
s.check(marks_db(meeting_id)[0][1] == "", "Esc keeps the mark without a label")
s.js("document.querySelector('[data-testid=mark-label]').click()")
s.wait_for("return document.querySelector('[data-testid=mark-label-input]')", 3)
s.js(set_value, "Budget decision", "Enter")
time.sleep(0.6)
s.check(marks_db(meeting_id)[0][1] == "Budget decision", f"Enter saves the label: {marks_db(meeting_id)}")
label = s.wait_for("return [...document.querySelectorAll('[data-testid=mark-label]')].map(e => e.textContent).join('|')", 3)
s.check(label.startswith("Budget decision"), f"the label shows: {label!r}")
themed(s, "meeting-marks")
s.js("[...document.querySelectorAll('[data-testid=mark-delete]')][1].click()")
time.sleep(0.6)
s.check(len(marks_db(meeting_id)) == 1, "a mark can be deleted")
s.js("document.querySelector('[data-testid=mark-time]').click()")
time.sleep(1.0)
pos = s.js("return document.querySelector('[data-testid=audio]').currentTime")
s.js("document.querySelector('[data-testid=play-toggle]').click()")
s.check(pos >= rows[0][0] / 1000 - 0.2, f"clicking a mark's time plays from there ({pos:.2f} s)")

# Settings > Shortcuts.
s.js("[...document.querySelectorAll('nav ul button')].find(b => b.textContent.trim() == 'Settings').click()")
keys = s.wait_for("return [...document.querySelectorAll('[data-testid=shortcut-row] kbd')].map(k => k.textContent).join(' ')", 5)
s.check(keys == "Ctrl+Alt+R Ctrl+Alt+M", f"Settings lists the shortcuts: {keys!r}")
s.check(bool(s.js("return document.querySelector('[data-testid=shortcuts-on]')")), "shortcuts are off until turned on")
s.js("document.querySelector('[data-testid=shortcuts-settings]').scrollIntoView({block: 'center'})")
themed(s, "settings-shortcuts-off")
if BIND:
    s.js("document.querySelector('[data-testid=shortcuts-on]').click()")
    on = s.wait_for("return document.querySelector('[data-testid=shortcuts-off]')", 120)
    msg = s.js("return document.querySelector('[data-testid=shortcuts-message]')?.textContent")
    s.check(bool(on), f"shortcuts turned on (message: {msg})")
    themed(s, "settings-shortcuts-on")
    s.js("document.querySelector('[data-testid=shortcuts-off]').click()")
    s.check(bool(s.wait_for("return document.querySelector('[data-testid=shortcuts-on]')", 10)), "and off again")
s.close()

# After a restart the label and the remaining mark are still there.
earlier = s.failures
s = Session(APP)
s.failures = earlier
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
s.js("document.querySelector('[data-testid=open-meeting]').click()")
label = s.wait_for("return document.querySelector('[data-testid=mark-label]')?.textContent", 10)
s.check(label == "Budget decision", f"after a restart the mark is there: {label!r}")

if SPEECH:
    # Real speech: marks among the transcript's blocks.
    s.js("document.querySelector('[data-testid=back]').click()")
    s.wait_for("return document.querySelector('[data-testid=new-recording]')", 10)
    player = subprocess.Popen(["paplay", "--volume=20000", SPEECH])
    s.js("document.querySelector('[data-testid=record-nav]').click()")
    s.wait_for("return document.querySelectorAll('[data-testid=source-chip]').length", 10)
    mid = s.invoke("current_recording")["meeting_id"]
    s.wait_for("return document.querySelector('[data-testid=caption-block]')", 30)
    s.check(bool(s.js("return document.querySelector('[data-testid=caption-block]')")), "captions appear")
    s.js("document.querySelector('[data-testid=mark-moment]').click()")
    time.sleep(6)
    s.invoke("mark_moment")
    order = s.wait_for("const r = [...document.querySelectorAll('[data-testid=mark], [data-testid=caption-block]')]"
                       ".map(e => e.dataset.testid); return r.includes('mark') && r.join(' ')", 5)
    s.check(bool(order), f"marks among the live blocks: {order}")
    themed(s, "live-marks-speech")
    time.sleep(6)
    player.terminate()
    s.js("document.querySelector('[data-testid=stop]').click()")
    s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 10)
    con = sqlite3.connect(DATA / "manent.db")
    folder = con.execute("SELECT dir FROM meetings WHERE id = ?", (mid,)).fetchone()[0]
    con.close()
    def status():
        con = sqlite3.connect(DATA / "manent.db")
        v = con.execute("SELECT status FROM meetings WHERE id = ?", (mid,)).fetchone()[0]
        con.close()
        return v
    end = time.time() + 180
    while status() != "done" and time.time() < end:
        time.sleep(1)
    s.check(status() == "done", f"final pass done ({status()})")
    n = s.wait_for("return document.querySelectorAll('[data-testid=mark]').length", 10)
    blocks = s.wait_for("return document.querySelectorAll('[data-testid=caption-block]').length", 10)
    s.check(n == 2 and bool(blocks), f"after the final pass: {n} marks among {blocks} blocks")
    themed(s, "meeting-marks-speech")
    rows = marks_db(mid)
    s.js("document.querySelector('[data-testid=mark-label]').click()")
    s.wait_for("return document.querySelector('[data-testid=mark-label-input]')", 3)
    s.js(set_value, "Follow up", "Enter")
    md = Path(folder) / "transcript.md"
    end = time.time() + 10
    while time.time() < end and "Follow up" not in (md.read_text() if md.is_file() else ""):
        time.sleep(0.3)
    text = md.read_text() if md.is_file() else ""
    s.check("marks: [{" in text and "> Marked at" in text and "Follow up" in text,
            "transcript.md lists the marks, also a new label")
    print("\n".join("     " + l for l in text.splitlines() if l.startswith(("marks:", "> Marked"))))
    for fmt, ext, want in (("md", "md", "> Marked at"), ("txt", "txt", "] Marked"), ("json", "json", '"marks"')):
        out = OUT / f"export-marks.{ext}"
        r = s.invoke("export_meeting", {"id": mid, "format": fmt, "path": str(out), "dateLabel": "test"})
        written = out.read_text() if out.is_file() else ""
        s.check(not (r or {}).get("error") and want in written and "Follow up" in written, f"{fmt} export has the marks ({r})")
    marks_json = json.loads((OUT / "export-marks.json").read_text())["marks"]
    s.check(len(marks_json) == len(rows), f"JSON marks: {marks_json}")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
