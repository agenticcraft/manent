"""Settings > Transcription and Storage smoke check on the real app (VRT-27).

Usage: python3 scripts/smoke-storage.py <app binary> <data dir> <screenshot dir> <speech.wav> <move-to dir>

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a temp folder with the engine and model
downloaded and some meetings. Checks: the update interval is saved and used by the next
recording; "Check model" finds a damaged file, which Download then restores; "Remove engine and
model" deletes them (they are backed up with hard links first and put back afterwards);
moving the meetings folder keeps meetings playable and exportable and back again; "playback
file only" drops a new import's tracks after transcription and Re-transcribe then explains why.
The folder dialog itself can't be driven, so moves use the command behind it.
"""
import os
import shutil
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SPEECH, MOVE_TO = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5])
LOG = Path(os.environ.get("TAURI_DRIVER_LOG", ""))


def open_page(s, name):
    s.js(f"[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('{name}')).click()")


def wait_jobs(s, timeout=300):
    end = time.time() + timeout
    while time.time() < end:
        if all(j["stage"] in ("done", "failed") for j in s.invoke("list_jobs")):
            return True
        time.sleep(1)
    return False


s = Session(APP)
s.wait_for("return document.querySelectorAll('nav[aria-label=Main] button').length >= 3 && true")
open_page(s, "Settings")
s.wait_for("return document.querySelector('[data-testid=rolling-interval]')", 10)

# The update interval.
s.js("const el = document.querySelector('[data-testid=rolling-interval]');"
     "Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, '30');"
     "el.dispatchEvent(new Event('change', {bubbles: true}));")
time.sleep(0.5)
s.check(s.invoke("get_app_settings")["rolling_interval_s"] == 30, "update interval saved (30 s)")
s.screenshot(OUT / "settings-transcription-dark.png")

# Check model: intact, then a damaged NOTICE is found, removed and downloaded again.
s.js("document.querySelector('[data-testid=engine-check]').click()")
res = s.wait_for("const r = document.querySelector('[data-testid=engine-result]'); return r && r.textContent", 60)
s.check(res == "Engine and model are intact.", f"check: {res!r}")
notice = DATA / "models" / "NOTICE-parakeet-bundle-standard.txt"
notice.write_text(notice.read_text() + "tampered")
s.js("document.querySelector('[data-testid=engine-check]').click()")
res = s.wait_for("const r = document.querySelector('[data-testid=engine-result]'); return r && r.textContent.includes('Damaged') && r.textContent", 60)
st = s.invoke("engine_status")
s.check(bool(res) and "NOTICE" in res and not st["model_installed"], f"a damaged file is found and removed: {res!r}")
s.js("document.querySelector('[data-testid=engine-download]').click()")
back = s.wait_for("const e = document.querySelector('[data-testid=engine-state]'); return e && e.textContent", 60)
s.check(bool(back) and s.invoke("engine_status")["model_installed"], "Download restores it")

# Remove engine and model (backed up with hard links, restored after).
backup = DATA.parent / (DATA.name + "-engine-backup")
shutil.rmtree(backup, ignore_errors=True)
for sub in ["engine", "models"]:
    shutil.copytree(DATA / sub, backup / sub, copy_function=os.link)
s.js("document.querySelector('[data-testid=engine-remove]').click()")
s.wait_for("return document.querySelector('[data-testid=engine-remove-confirm]')", 3)
s.js("document.querySelector('[data-testid=engine-remove-confirm]').click()")
offer = s.wait_for("return document.querySelector('[data-testid=engine-download]') && true", 10)
s.check(bool(offer) and not (DATA / "engine").exists() and not (DATA / "models").exists(),
        "engine and model removed; Download offered again")
s.screenshot(OUT / "settings-engine-removed.png")
for sub in ["engine", "models"]:
    shutil.copytree(backup / sub, DATA / sub, copy_function=os.link)
shutil.rmtree(backup)
s.close()
failures = list(s.failures)

# Restart so the restored engine is picked up; record briefly to see the interval used.
s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
s.check(s.invoke("engine_status")["model_installed"], "engine and model restored")
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
time.sleep(2)
meeting = s.invoke("current_recording")["meeting_id"]
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 30)  # Stop opens the recording
s.js("document.querySelector('[data-testid=back]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 10)
if LOG.is_file():
    s.check(f"meeting {meeting}: rolling passes every 30 s" in LOG.read_text(errors="replace"),
            "the next recording used the 30 s interval")
wait_jobs(s)
s.invoke("set_rolling_interval", {"seconds": 45})

# Move the meetings folder and back.
before = s.invoke("get_app_settings")["meetings_dir"]
moved = s.invoke("move_meetings", {"to": str(MOVE_TO)})
s.check(isinstance(moved, dict) and moved["meetings_dir"] == str(MOVE_TO), f"meetings moved to {moved}")
con = sqlite3.connect(DATA / "manent.db")
dirs = [d for (d,) in con.execute("SELECT dir FROM meetings")]
paths = [p for (p,) in con.execute("SELECT path FROM tracks")]
con.close()
s.check(all(d.startswith(str(MOVE_TO)) for d in dirs) and all(p.startswith(str(MOVE_TO)) for p in paths),
        f"database points to the new folder ({len(dirs)} meetings, {len(paths)} tracks)")
open_page(s, "Settings")
shown = s.wait_for("const d = document.querySelector('[data-testid=meetings-dir]'); return d && d.textContent", 5)
s.check(shown == str(MOVE_TO), f"Settings shows the new folder ({shown})")
s.screenshot(OUT / "settings-storage-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.5)
s.screenshot(OUT / "settings-storage-light.png")
s.js("document.documentElement.classList.add('dark')")
open_page(s, "Library")
s.wait_for("return document.querySelector('[data-testid=open-meeting]')", 5)
first = s.invoke("list_meetings")[0]["id"]
s.js("document.querySelector('[data-testid=open-meeting]').click()")
dur = s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 10)
s.check(bool(dur), f"a moved meeting still plays ({dur} s)")
out = MOVE_TO.parent / "moved-export.txt"
r = s.invoke("export_meeting", {"id": first, "format": "txt", "path": str(out), "dateLabel": "x"})
s.check(r is None and out.is_file(), "and exports")
s.js("document.querySelector('[data-testid=back]').click()")
back = s.invoke("move_meetings", {"to": None})
s.check(back["meetings_dir"] == before and not any(MOVE_TO.iterdir()), "moved back to the default folder")

# Playback file only: a new import keeps only mix.opus.
s.invoke("set_keep_audio", {"keep": "mix"})
[job] = s.invoke("import_audio", {"paths": [str(SPEECH)]})
s.check(wait_jobs(s), "import transcribed")
job = next(j for j in s.invoke("list_jobs") if j["id"] == job)
con = sqlite3.connect(DATA / "manent.db")
mdir = con.execute("SELECT dir FROM meetings WHERE id = ?", (job["meeting_id"],)).fetchone()[0]
tracks = [p for (p,) in con.execute("SELECT path FROM tracks WHERE meeting_id = ?", (job["meeting_id"],))]
con.close()
files = sorted(p.name for p in Path(mdir).iterdir())
s.check(job["stage"] == "done" and not any(Path(t).exists() for t in tracks) and "mix.opus" in files and "waveform.json" in files,
        f"tracks dropped, playback file and waveform kept ({files})")
r = s.invoke("retranscribe", {"meetingId": job["meeting_id"]})
s.check(isinstance(r, dict) and "not kept" in r.get("error", ""), f"Re-transcribe explains why it can't ({r})")
s.invoke("set_keep_audio", {"keep": "all"})
s.close()
failures += s.failures
print(f"\n{'PASS' if not failures else 'FAILED: ' + '; '.join(failures)}; screenshots in {OUT}")
sys.exit(1 if failures else 0)
