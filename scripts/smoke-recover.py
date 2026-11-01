"""Recovering a recording cut off by a crash, on the real app (VRT-41). Needs audio hardware
and the engine.

Usage: python3 scripts/smoke-recover.py <app binary> <data dir> <screenshot dir> <speech.wav>

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (engine and model downloaded). Records
while the WAV plays (60% volume; use a loudness-normalised file), kills the app with SIGKILL
after 20 s, and starts it again: the meeting must be recovered (tracks repaired, playback file,
duration), transcribed, and show that it was interrupted. Meetings left in 'recording' by
earlier runs are recovered too.
"""
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SPEECH = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])


def pids():
    return subprocess.run(["pgrep", "-x", Path(APP).name], capture_output=True, text=True).stdout.split()


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
meeting = s.invoke("current_recording")["meeting_id"]
player = subprocess.Popen(["paplay", "--volume=39000", str(SPEECH)])
time.sleep(20)
for pid in pids():
    subprocess.run(["kill", f"-{signal.SIGKILL.value}", pid])
player.terminate()
try:
    s.close()
except RuntimeError:
    pass  # the app is gone

con = sqlite3.connect(DATA / "veritas.db")
cut_off = [m for (m,) in con.execute("SELECT id FROM meetings WHERE status = 'recording'")]
con.close()
s.check(meeting in cut_off, f"killed during a recording: meetings left in 'recording': {cut_off}")

s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
seen_recover = s.wait_for("return [...document.querySelectorAll('[data-testid=jobs] li')].some(li => li.textContent.includes('Recovering'))", 10)
s.check(bool(seen_recover), "the Library shows the recovery job")
end = time.time() + 300
while time.time() < end and any(j["stage"] not in ("done", "failed") for j in s.invoke("list_jobs")):
    time.sleep(1)
failed = [j for j in s.invoke("list_jobs") if j["stage"] == "failed"]
s.check(not failed, f"recovery jobs finished ({failed})")

con = sqlite3.connect(DATA / "veritas.db")
for m in cut_off:
    row = con.execute("SELECT status, duration_ms, interrupted, dir FROM meetings WHERE id = ?", (m,)).fetchone()
    if row is None:
        print(f"     meeting {m}: removed (no audio was saved)")
        continue
    status, duration, interrupted, mdir = row
    seconds = [x for (x,) in con.execute("SELECT seconds FROM tracks WHERE meeting_id = ?", (m,))]
    words = con.execute("SELECT COUNT(*) FROM segments WHERE meeting_id = ? AND pass = 'final'", (m,)).fetchone()[0]
    print(f"     meeting {m}: {status}, {duration} ms, tracks {seconds}, {words} final segments")
    s.check(status == "done" and interrupted == 1 and (Path(mdir) / "mix.opus").is_file()
            and seconds and all(abs(x * 1000 - duration) < 1000 for x in seconds),
            f"meeting {m} recovered: playable, duration from the repaired tracks")
row = con.execute("SELECT duration_ms FROM meetings WHERE id = ?", (meeting,)).fetchone()
words = con.execute("SELECT COUNT(*) FROM segments WHERE meeting_id = ? AND pass = 'final' AND text != ''", (meeting,)).fetchone()[0]
con.close()
s.check(row is not None and 18_000 < row[0] < 22_000, f"the killed recording kept ~20 s ({row})")
s.check(words > 0, f"and was transcribed ({words} segments)")

s.js(f"document.querySelector(\"[data-testid=meeting-row][data-meeting-id='{meeting}'] [data-testid=open-meeting]\").click()")
note = s.wait_for("const n = document.querySelector('[data-testid=interrupted-note]'); return n && n.textContent", 10)
s.check(bool(note) and "quit unexpectedly" in note, f"the meeting says it was recovered: {note!r}")
dur = s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 10)
s.check(bool(dur) and abs(dur - row[0] / 1000) < 1.5, f"it plays ({dur} s)")
s.screenshot(OUT / "recovered-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.5)
s.screenshot(OUT / "recovered-light.png")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
