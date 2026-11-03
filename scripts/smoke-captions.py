"""Live captions smoke check on the real app (VRT-16). Needs audio hardware and the engine.

Usage: python3 scripts/smoke-captions.py <app binary> <data dir> <screenshot dir> <speech.wav>

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a temp folder in which the engine and
model are already downloaded (e.g. by scripts/smoke-engine.py). The WAV is played on the
default output (at 60% volume, so use a loudness-normalised file) during a recording; if `<speech>.truth.json` exists (segments with start, end,
lang, text; e.g. spikes/localai/samples/meeting-nl-en.wav) the system-audio captions are
scored per language. Prints when each word appeared after it was spoken.
"""
import json
import re
import sqlite3
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SPEECH = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
TRUTH = SPEECH.with_suffix(".truth.json")


def norm(text):
    return re.sub(r"[^\w\s']", " ", text.lower()).split()


def wer(ref, hyp):
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / max(len(ref), 1)


def duration(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
                         capture_output=True, text=True, check=True).stdout
    return float(out)


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
status = s.invoke("engine_status")
if not (status["engine_installed"] and status["model_installed"]):
    sys.exit("engine/model not downloaded in this data folder (run smoke-engine.py first)")

s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
session = s.invoke("current_recording")
s.check(session and session["captions"], f"captions on for the recording ({session and session['captions']})")
placeholder = s.js("const p = document.querySelector('[data-testid=live-placeholder]'); return p && p.textContent")
s.check(bool(placeholder) and "Listening" in placeholder, f"waiting state shown: {placeholder!r}")
meeting = session["meeting_id"]
started = session["started_at"] / 1000

time.sleep(1.0)
play_at = time.time()
player = subprocess.Popen(["paplay", "--volume=39000", str(SPEECH)])
seen = {}  # (track_id, start_ms) -> (first seen wall time, caption)
end = play_at + duration(SPEECH) + 12
while time.time() < end:
    for c in s.invoke("live_transcript", {"meetingId": meeting}):
        if c["pass"] == "caption":
            seen.setdefault((c["track_id"], c["start_ms"]), (time.time(), c))
    time.sleep(0.25)
player.wait()
s.screenshot(OUT / "live-captions-dark.png")

# Scroll up: following stops and the pill appears; the pill jumps back. A short viewport
# makes sure there is something to scroll.
s.js("document.querySelector('[data-testid=live-transcript]').style.maxHeight = '200px'")
time.sleep(0.3)
s.js("document.querySelector('[data-testid=live-transcript]').scrollTop = 0;"
     "document.querySelector('[data-testid=live-transcript]').dispatchEvent(new Event('scroll'))")
pill = s.wait_for("return document.querySelector('[data-testid=jump-to-live]') && true", 3)
s.check(bool(pill), "'Jump to live' appears after scrolling up")
s.screenshot(OUT / "live-captions-scrolled.png")
s.js("document.querySelector('[data-testid=jump-to-live]').click()")
gone = s.wait_for("const el = document.querySelector('[data-testid=live-transcript]');"
                  "return !document.querySelector('[data-testid=jump-to-live]') && el.scrollHeight - el.scrollTop - el.clientHeight < 50", 3)
s.check(bool(gone), "'Jump to live' scrolls back to the newest text")
s.js("document.querySelector('[data-testid=live-transcript]').style.maxHeight = ''")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "live-captions-light.png")
s.js("document.documentElement.classList.add('dark')")

blocks = s.js("return [...document.querySelectorAll('[data-testid=caption-block]')].map(b => b.dataset.kind)")
s.check(len(blocks) > 0 and "loopback" in blocks, f"caption blocks shown: {len(blocks)} ({sorted(set(blocks))})")

s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 30)
time.sleep(3)  # the last chunk is transcribed after Stop (the final pass runs on)
s.close()

con = sqlite3.connect(DATA / "manent.db")
tracks = dict(con.execute("SELECT id, source_kind FROM tracks WHERE meeting_id = ?", (meeting,)).fetchall())
stored = con.execute("SELECT track_id FROM segments WHERE meeting_id = ?", (meeting,)).fetchall()
con.close()
# Captions are replaced by the rolling and final passes (VRT-17); score them as seen live.
rows = [(c["track_id"], c["start_ms"], c["end_ms"], c["text"], json.dumps(c["words"])) for _, c in seen.values()]
s.check(len(seen) > 0 and all(c["track_id"] in tracks for _, c in seen.values()) and stored,
        f"{len(seen)} captions seen live with their track; {len(stored)} segments stored")

# Latency: when each word was first shown, after it was spoken (track time from started_at).
lat = []
for (track_id, _), (t_seen, c) in seen.items():
    for w in c["words"]:
        lat.append(t_seen - (started + w["end_ms"] / 1000))
lat.sort()
med, p90, worst = statistics.median(lat), lat[int(len(lat) * 0.9)], lat[-1]
print(f"     latency over {len(lat)} words: median {med:.1f} s, p90 {p90:.1f} s, max {worst:.1f} s")
# A chunk that came back empty is retried as part of a longer one, so its words can take
# up to ~10 s; that is the exception, hence p90 rather than the maximum.
s.check(med <= 4.5 and p90 <= 8.0, f"text appears within ~4 s median ({med:.1f}) and ~8 s p90 ({p90:.1f}) of speech")

if TRUTH.exists():
    truth = json.load(open(TRUTH))
    offset = (play_at - started) * 1000
    loop_words = [w for r in rows if tracks[r[0]] == "loopback" for w in json.loads(r[4] or "[]")]
    totals = {}
    for seg in truth["segments"]:
        a, b = offset + seg["start"] * 1000 - 600, offset + seg["end"] * 1000 + 600
        hyp = " ".join(w["text"] for w in sorted(loop_words, key=lambda w: w["start_ms"]) if a <= w["start_ms"] <= b)
        ref = norm(seg["text"])
        t = totals.setdefault(seg["lang"], [0.0, 0])
        t[0] += wer(ref, norm(hyp)) * len(ref)
        t[1] += len(ref)
        print(f"     {seg['lang']} {wer(ref, norm(hyp)):5.0%}  {hyp[:90]}")
    for lang, (errs, n) in totals.items():
        s.check(errs / n <= 0.25, f"{lang} live WER {errs / n:.1%} on the system track")

print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
