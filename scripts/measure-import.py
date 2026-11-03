"""Measure a long import on the real app (VRT-35): job time, memory, quality, meeting screen.

Usage: python3 scripts/measure-import.py <app binary> <data dir> <screenshot dir> <audio file> [<captions.vtt>]
(MEETING=<id> skips the import and measures that meeting.)

tauri-driver must run with MANENT_DATA_DIR=<data dir> (with the engine and model). Imports the
file and, while the job runs, samples the resident memory of the app and the engine helper
every 2 s. Then reports the job time against the audio length, the word disagreement with the
captions (YouTube auto-captions are ASR output themselves: a rough reference), and the meeting
screen: time until the transcript is shown, a scroll to the end, and the playback highlight
near the end of the meeting.
"""
import os
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, AUDIO = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]).resolve()
VTT = Path(sys.argv[5]) if len(sys.argv) > 5 else None


def rss_mb(name):
    """Resident and peak memory (MB) of the process named `name`, or (0, 0)."""
    pids = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    best = (0, 0)
    for pid in pids:
        try:
            status = Path(f"/proc/{pid}/status").read_text()
        except OSError:
            continue
        get = lambda key: int(re.search(rf"{key}:\s+(\d+)", status).group(1)) // 1024  # noqa: E731
        best = max(best, (get("VmRSS"), get("VmHWM")))
    return best


def norm(text):
    return re.sub(r"[^\w\s']", " ", text.lower().replace("-", " ")).split()


def wer(ref, hyp):
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / max(len(ref), 1)


def vtt_text(path, until_s):
    """Plain text of the cues starting before `until_s` in a YouTube auto-caption VTT."""
    lines, last, t = [], None, 0.0
    for line in open(path, encoding="utf-8"):
        m = re.match(r"(\d+):(\d+):([\d.]+) -->", line)
        if m:
            t = int(m[1]) * 3600 + int(m[2]) * 60 + float(m[3])
            continue
        line = re.sub(r"<[^>]+>", "", line).strip()
        if t >= until_s or not line or line.startswith(("WEBVTT", "Kind:", "Language:")) or line == last:
            continue
        lines.append(line)
        last = line
    out = [x for i, x in enumerate(lines) if i + 1 == len(lines) or not lines[i + 1].startswith(x)]
    return re.sub(r"\[[^\]]*\]", " ", " ".join(out))


seconds = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(AUDIO)],
                               capture_output=True, text=True, check=True).stdout)
s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=import-audio]')", 15)
if os.environ.get("MEETING"):
    meeting = int(os.environ["MEETING"])  # measure an earlier import again
else:
    start = time.time()
    [job_id] = s.invoke("import_audio", {"paths": [str(AUDIO)]})
    peak_app = peak_helper = 0
    samples = []
    while True:
        job = next(j for j in s.invoke("list_jobs") if j["id"] == job_id)
        app_rss, _ = rss_mb(Path(APP).name)
        helper_rss, helper_hwm = rss_mb("manent-engine")
        peak_app, peak_helper = max(peak_app, app_rss), max(peak_helper, helper_hwm)
        samples.append((round(time.time() - start), job["stage"], round(job["progress"] * 100), app_rss, helper_rss))
        if job["stage"] in ("done", "failed"):
            break
        time.sleep(2)
    took = time.time() - start
    print(f"     {AUDIO.name}: {seconds / 60:.1f} min of audio, job {took:.0f} s (x{seconds / took:.1f} real time), stage {job['stage']} {job['error'] or ''}")
    for row in samples[:: max(1, len(samples) // 12)]:
        print(f"       t={row[0]:>4}s {row[1]:<12} {row[2]:>3}%  app {row[3]:>5} MB  helper {row[4]:>5} MB")
    print(f"     peak memory: app {peak_app} MB, helper {peak_helper} MB")
    s.check(job["stage"] == "done", "import done")
    meeting = job["meeting_id"]

segments = s.invoke("live_transcript", {"meetingId": meeting})
words = sum(len(x["words"]) for x in segments)
text = " ".join(x["text"] for x in sorted(segments, key=lambda x: x["start_ms"]))
labels = sorted({x["label"] for x in segments if x["label"]})
print(f"     transcript: {len(segments)} blocks, {words} words, speakers {labels}")
if VTT:
    ref = norm(vtt_text(VTT, seconds))
    print(f"     disagreement with the captions: {wer(ref, norm(text)):.1%} over {len(ref)} caption words")

# The meeting screen with the whole transcript.
t0 = time.time()
s.js("document.querySelector(`[data-testid=meeting-row][data-meeting-id='${arguments[0]}'] [data-testid=open-meeting]`).click()", meeting)
shown = s.wait_for("return document.querySelectorAll('[data-testid=word]').length >= arguments[0] * 0.9"
                   " && document.querySelectorAll('[data-testid=word]').length", 60, words)
t_open = time.time() - t0
s.check(bool(shown), f"meeting screen shows {shown} words after {t_open:.1f} s")
t0 = time.time()
s.js("const m = document.querySelector('main'); m.scrollTop = m.scrollHeight;")
time.sleep(0.2)
at_end = s.js("const m = document.querySelector('main'); return m.scrollHeight - m.scrollTop - m.clientHeight < 5")
print(f"     scrolled to the end: {at_end} ({time.time() - t0:.2f} s)")
s.wait_for("const a = document.querySelector('[data-testid=audio]'); return a && a.readyState >= 1 && a.duration", 20)
target = s.js("const w = [...document.querySelectorAll('[data-testid=word]')]; const x = w[w.length - 40];"
              "x.scrollIntoView({block: 'center'}); return [Number(x.dataset.wordMs), x.textContent]")
time.sleep(0.3)
s.js("const w = [...document.querySelectorAll('[data-testid=word]')]; w[w.length - 40].click()")
time.sleep(2)
state = s.js("const a = document.querySelector('[data-testid=audio]'); const w = document.querySelector('[data-testid=word][data-active=true]');"
             "return [a.currentTime, w && Number(w.dataset.wordMs)]")
s.check(state[1] is not None and abs(state[0] * 1000 - state[1]) < 3000 and state[0] * 1000 >= target[0],
        f"near the end: clicking '{target[1]}' at {target[0] / 1000:.0f} s plays and highlights ({state})")
s.screenshot(OUT / f"{AUDIO.stem}-meeting.png")
s.js("document.querySelector('[data-testid=play-toggle]').click()")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}")
sys.exit(1 if s.failures else 0)
