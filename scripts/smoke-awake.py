"""Keeping the computer awake while recording, and audio gaps, on the real app (VRT-36).

Usage: python3 scripts/smoke-awake.py <app binary> <data dir> <screenshot dir>

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (Linux with systemd; audio hardware).
Checks that logind lists Veritas's sleep inhibitor while a recording runs and drops it when
the recording stops and when the app is killed (a crash). A real suspend can't be driven from
a script, so the gap marker is checked by adding a gap to a stopped recording in the database
(the detection itself is unit-tested in audio::recorder) and opening that meeting.
"""
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])


def inhibitors():
    out = subprocess.run(["systemd-inhibit", "--list"], capture_output=True, text=True).stdout
    return [line for line in out.splitlines() if "Veritas" in line and "Recording a meeting" in line]


def wait_until(cond, timeout=5.0):
    end = time.time() + timeout
    while time.time() < end:
        if cond():
            return True
        time.sleep(0.1)
    return cond()


def record(s, seconds):
    s.js("document.querySelector('[data-testid=new-recording]').click()")
    s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
    time.sleep(seconds)
    return s.invoke("current_recording")["meeting_id"]


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
s.check(not inhibitors(), "no inhibitor before recording")
meeting = record(s, 3)
held = inhibitors()
s.check(len(held) == 1 and "sleep" in held[0] and "block" in held[0], f"inhibitor while recording: {held}")
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 30)
s.check(wait_until(lambda: not inhibitors()), "released when the recording stops")

# A gap: the meeting shows a marker at that point, between the blocks; also without words.
con = sqlite3.connect(DATA / "veritas.db")
[(talk,)] = con.execute("""SELECT s.meeting_id FROM segments s JOIN meetings m ON m.id = s.meeting_id
                           WHERE s.pass = 'final' AND m.duration_ms > 180000 ORDER BY s.meeting_id DESC LIMIT 1""")
con.execute("DELETE FROM meeting_gaps WHERE meeting_id = ?", (talk,))  # from an earlier run
con.execute("INSERT INTO meeting_gaps (meeting_id, at_ms, missing_ms) VALUES (?, 1500, 420000)", (meeting,))
con.execute("INSERT INTO meeting_gaps (meeting_id, at_ms, missing_ms) VALUES (?, 60000, 420000)", (talk,))
con.commit()
con.close()
s.check(s.invoke("meeting_gaps", {"meetingId": meeting}) == [{"at_ms": 1500, "missing_ms": 420000}], "gap returned")


def open_meeting(m):
    s.js(f"document.querySelector(\"[data-testid=meeting-row][data-meeting-id='{m}'] [data-testid=open-meeting]\").click()")


open_meeting(meeting)
text = s.wait_for("const g = document.querySelector('[data-testid=audio-gap]'); return g && g.textContent", 10)
s.check(text == "No audio for 7 min at 0:02: the computer was asleep", f"marker shown without a transcript: {text!r}")
s.js("document.querySelector('[data-testid=back]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 10)
open_meeting(talk)
s.wait_for("return document.querySelector('[data-testid=meeting-transcript] [data-testid=audio-gap]')", 10)
place = s.wait_for("const g = document.querySelector('[data-testid=meeting-transcript] [data-testid=audio-gap]'); if (!g) return null;"
                   "g.scrollIntoView({block: 'center'});"
                   "const p = g.previousElementSibling, n = g.nextElementSibling;"
                   "return [p && Number(p.dataset.blockStart), n && Number(n.dataset.blockStart)]", 10)
s.check(bool(place) and None not in place and place[0] <= 60000 < place[1],
        f"marker between the blocks around 1:00 ({place})")
time.sleep(0.3)
s.screenshot(OUT / "gap-marker-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.5)
s.screenshot(OUT / "gap-marker-light.png")
s.js("document.documentElement.classList.add('dark')")
s.js("document.querySelector('[data-testid=back]').click()")

# A crash while recording: the inhibitor ends with the app.
record(s, 2)
s.check(len(inhibitors()) == 1, "inhibitor while recording (again)")
app_pid = subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True, text=True).stdout.split()
for pid in app_pid:
    subprocess.run(["kill", f"-{signal.SIGKILL.value}", pid])
s.check(bool(app_pid) and wait_until(lambda: not inhibitors()), f"released when the app is killed ({app_pid})")
try:
    s.close()
except RuntimeError:
    pass  # the app is gone
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
