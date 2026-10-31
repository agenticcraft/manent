"""Engine smoke check on the real app: download, cancel/resume, start, crash recovery.

Usage: python3 scripts/smoke-engine.py <app binary> <data dir> <screenshot dir> [--seed-model <gguf>]

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (an empty temp folder). Downloads the
engine (~14 MB) from quay.io. With --seed-model, all but the last 40 MB of a local copy of the
model are put in place as a partial download first, so the model download resumes with a Range
request and still has to pass the sha256 check (instead of fetching 1.1 GB). Needs audio
hardware for the recording part.
"""
import hashlib
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
SEED = sys.argv[sys.argv.index("--seed-model") + 1] if "--seed-model" in sys.argv else None
MODEL = "parakeet-bundle-standard.gguf"
MODEL_SHA = "85304e3bb49de04b84a7d0c76baaa7dedd1a0c02c0920078a99f92c00cffbdf6"


def helper_pids():
    """veritas-engine processes that belong to this data folder."""
    out = subprocess.run(["pgrep", "-f", f"veritas-engine.*{DATA}"], capture_output=True, text=True).stdout
    return [int(p) for p in out.split()]


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(1 << 22):
            h.update(chunk)
    return h.hexdigest()


if SEED:
    models = DATA / "models"
    models.mkdir(parents=True, exist_ok=True)
    keep = Path(SEED).stat().st_size - 40 * 1024 * 1024
    with open(SEED, "rb") as src, open(models / f"{MODEL}.part", "wb") as dst:
        while keep > 0:
            chunk = src.read(min(1 << 22, keep))
            dst.write(chunk)
            keep -= len(chunk)
    print(f"     seeded {MODEL}.part with {(models / f'{MODEL}.part').stat().st_size / 1e6:.0f} MB")

s = Session(APP)
s.wait_for("return document.querySelectorAll('nav[aria-label=Main] button').length === 3 && true")


def open_page(name):
    s.js(f"[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('{name}')).click()")


def state():
    return s.js("const e = document.querySelector('[data-testid=engine-state]'); return e && e.textContent")


open_page("Settings")
offer = s.wait_for("const b = document.querySelector('[data-testid=engine-download]'); return b && b.closest('div.rounded-lg').textContent", 10)
s.check(bool(offer) and "One-time download" in offer, f"download offered: {offer!r}")
size_before = s.invoke("engine_status")["download_size"]
s.screenshot(OUT / "engine-download.png")

# Start, cancel part-way through the model download (mid-transfer), then resume.
s.js("document.querySelector('[data-testid=engine-download]').click()")
got = s.wait_for("const p = document.querySelector('[data-testid=engine-progress]'); return p && p.textContent", 30)
s.check(bool(got), f"progress shown: {got!r}")
seeded = (DATA / "models" / f"{MODEL}.part").stat().st_size if SEED else 0
end = time.time() + 300
while time.time() < end:
    d = s.invoke("engine_status")["download"]
    if d and d["item"] == "model" and d["received"] > seeded + 5_000_000:
        break
    time.sleep(0.1)
s.js("const c = document.querySelector('[data-testid=engine-cancel]'); c && c.click()")
cancelled = s.wait_for("return document.querySelector('[data-testid=engine-download]') && true", 15)
status = s.invoke("engine_status")
part = DATA / "models" / f"{MODEL}.part"
kept = part.stat().st_size if part.exists() else 0
s.check(bool(cancelled) and status["engine_installed"] and not status["model_installed"] and kept > seeded + 5_000_000,
        f"cancel mid-model keeps the part ({kept / 1e6:.0f} MB, seeded {seeded / 1e6:.0f} MB);"
        f" still to download {status['download_size'] / 1e6:.1f} MB (was {size_before / 1e6:.1f})")

s.js("document.querySelector('[data-testid=engine-download]').click()")
first = None
end = time.time() + 30
while time.time() < end and first is None:
    d = s.invoke("engine_status")["download"]
    if d and d["item"] == "model":
        first = d["received"]
    time.sleep(0.05)
s.check(first is not None and first >= kept, f"download resumes at {first and first / 1e6:.0f} MB, not from zero")
s.screenshot(OUT / "engine-downloading.png")
t0 = time.time()
done = s.wait_for("const e = document.querySelector('[data-testid=engine-state]'); return e && e.textContent", 1800)
status = s.invoke("engine_status")
s.check(bool(done) and status["engine_installed"] and status["model_installed"] and not status["download_error"],
        f"engine and model installed in {time.time() - t0:.0f} s (error: {status['download_error']})")
engine_files = sorted(p.name for p in (DATA / "engine").iterdir())
s.check(engine_files == ["engine.json", "libgomp.so.1", "libparakeet.so"], f"engine folder holds only {engine_files}")
s.check(sha256(DATA / "models" / MODEL) == MODEL_SHA, "model sha256 matches the pin")
s.check(not list((DATA / "models").glob("*.part")), "no partial files left")

# Start the engine.
s.js("document.querySelector('[data-testid=engine-start]').click()")
ready = s.wait_for("const e = document.querySelector('[data-testid=engine-state]'); return e && e.textContent.startsWith('Running') && e.textContent", 60)
status = s.invoke("engine_status")
s.check(bool(ready), f"engine running: {ready!r}, info {status['helper_info']}")
s.check(status["helper_info"]["abi"] == 10 and status["helper_info"]["components"] == ["asr", "vad", "diar", "voice"],
        "ABI 10 with asr/vad/diar/voice")
s.check(len(helper_pids()) == 1, f"one helper process ({helper_pids()})")
s.js("document.querySelector('[data-testid=engine-licences]').click()")
notice = s.wait_for("const n = document.querySelector('[data-testid=engine-notice]'); return n && n.textContent", 5)
s.check(bool(notice) and "CC-BY-4.0" in notice and "OpenMDW" in notice, "model licences shown")
s.screenshot(OUT / "engine-ready-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "engine-ready-light.png")
s.js("document.documentElement.classList.add('dark')")

# Crash the helper during a recording: it restarts, the recording carries on.
open_page("Library")
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
s.js("document.querySelector('[data-testid=new-recording]').click()")
live = s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
s.check(bool(live), "recording started on the Live screen")
if not live:
    s.screenshot(OUT / "not-live.png")
rec_start = time.time()
time.sleep(2)
old = s.invoke("engine_status")["helper_pid"]
subprocess.run(["kill", "-9", str(old)], check=True)
restarted = None
end = time.time() + 30
while time.time() < end:
    st = s.invoke("engine_status")
    if st["helper"] == "ready" and st["helper_pid"] not in (None, old):
        restarted = st
        break
    time.sleep(0.3)
s.check(bool(restarted) and restarted["restarts"] == 1,
        f"killed helper {old} was restarted as {restarted and restarted['helper_pid']} (restarts {restarted and restarted['restarts']})")
ping = s.invoke("engine_start")
s.check("error" not in ping, "restarted engine answers")
timer = "const t = document.querySelector('[data-testid=recording-timer]'); return t ? t.textContent : 'not on the Live screen'"
timer1 = s.js(timer)
time.sleep(2)
timer2 = s.js(timer)
if timer1 == "not on the Live screen":
    s.screenshot(OUT / "left-live.png")
s.check(timer1 != timer2 and ":" in timer2, f"recording kept running ({timer1} -> {timer2})")
open_page("Settings")
shown = s.wait_for("const e = document.querySelector('[data-testid=engine-state]'); return e && e.textContent.includes('restarted') && e.textContent", 5)
s.check(bool(shown), f"restart shown in Settings: {shown!r}")
s.js("document.querySelector('[data-testid=recording-nav]').click()")
s.wait_for("return document.querySelector('[data-testid=stop]')", 5)
elapsed = time.time() - rec_start
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 20)
con = sqlite3.connect(DATA / "veritas.db")
duration_ms = con.execute("SELECT duration_ms FROM meetings").fetchone()[0]
tracks = con.execute("SELECT source_kind, seconds FROM tracks").fetchall()
con.close()
s.check(abs(duration_ms / 1000 - elapsed) < 1.5 and all(abs(sec - duration_ms / 1000) < 0.5 for _, sec in tracks),
        f"recording complete: {duration_ms / 1000:.1f} s (wall {elapsed:.1f} s), tracks {tracks}")

s.close()
time.sleep(2)
s.check(not helper_pids(), f"helper stopped with the app ({helper_pids()})")
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
