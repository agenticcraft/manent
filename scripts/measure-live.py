"""Measure a long live recording on the real app (VRT-35): do captions and rolling passes keep up?

Usage: python3 scripts/measure-live.py <app binary> <data dir> <screenshot dir> <speech.wav> [<captions.vtt>]

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (with the engine and model). Records
while the WAV plays on the default output (60% volume; use a loudness-normalised file). Every
2 s it notes new captions and the end of the newest rolling pass; every 10 s the resident
memory of the app and the engine helper. Reports per 5 minutes: the caption latency (when a
caption appeared after its last word was spoken), how far the rolling passes lag behind, and
memory. After Stop: the final pass time and, with captions, the disagreement of the final
transcript with them (YouTube auto-captions: a rough reference).
"""
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SPEECH = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
VTT = Path(sys.argv[5]) if len(sys.argv) > 5 else None


def rss_mb(name):
    pids = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    best = 0
    for pid in pids:
        try:
            best = max(best, int(re.search(r"VmRSS:\s+(\d+)", Path(f"/proc/{pid}/status").read_text()).group(1)) // 1024)
        except (OSError, AttributeError):
            pass
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


length = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(SPEECH)],
                              capture_output=True, text=True, check=True).stdout)
s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
session = s.invoke("current_recording")
meeting, started = session["meeting_id"], session["started_at"] / 1000
time.sleep(1.0)
play_at = time.time()
player = subprocess.Popen(["paplay", "--volume=39000", str(SPEECH)])
seen = {}  # (track, start_ms) -> latency in s
rows = []  # (minute, caption latencies, rolling lag s, app MB, helper MB)
window, last_mem = [], 0.0
next_report = play_at + 300
while player.poll() is None or time.time() < play_at + length + 5:
    now = time.time()
    segs = s.invoke("live_transcript", {"meetingId": meeting})
    for c in segs:
        if c["pass"] == "caption" and c["kind"] == "loopback" and (c["track_id"], c["start_ms"]) not in seen:
            lat = now - (started + c["end_ms"] / 1000)
            seen[(c["track_id"], c["start_ms"])] = lat
            window.append(lat)
    rolled = max((c["end_ms"] for c in segs if c["pass"] == "rolling"), default=0) / 1000
    if now - last_mem >= 10:
        last_mem = now
        mem = (rss_mb(Path(APP).name), rss_mb("veritas-engine"))
    if now >= next_report:
        rows.append((round((now - play_at) / 60), window, now - started - rolled, *mem))
        window, next_report = [], next_report + 300
        print(f"     {rows[-1][0]:>3} min: captions median {statistics.median(rows[-1][1]) if rows[-1][1] else 0:.1f} s "
              f"(max {max(rows[-1][1], default=0):.1f} s, {len(rows[-1][1])}), rolling covers up to {rows[-1][2]:.0f} s ago, "
              f"app {mem[0]} MB, helper {mem[1]} MB", flush=True)
    time.sleep(2)
if window:
    rows.append((round((time.time() - play_at) / 60), window, time.time() - started - rolled, *mem))
s.screenshot(OUT / "live-long-dark.png")
s.js("document.querySelector('[data-testid=stop]').click()")
stop = time.time()
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 60)
while any(j["stage"] not in ("done", "failed") for j in s.invoke("list_jobs")):
    time.sleep(2)
final_s = time.time() - stop
all_lat = list(seen.values())
print(f"     {length / 60:.1f} min played; {len(all_lat)} captions, latency median {statistics.median(all_lat):.1f} s, "
      f"p95 {sorted(all_lat)[int(len(all_lat) * 0.95)]:.1f} s")
print(f"     final pass {final_s:.0f} s after Stop")
s.check(all(statistics.median(r[1]) < 8 for r in rows if r[1]), "captions keep up in every 5 minutes (median < 8 s)")
s.check(all(r[2] < 150 for r in rows), "rolling passes keep up (cover all but the last 150 s)")
segs = s.invoke("live_transcript", {"meetingId": meeting})
s.check(segs and all(c["pass"] == "final" for c in segs), f"final transcript: {len(segs)} blocks")
text = " ".join(c["text"] for c in sorted(segs, key=lambda c: c["start_ms"]) if c["kind"] == "loopback")
if VTT:
    ref = norm(vtt_text(VTT, length))
    print(f"     final transcript (system track) vs captions: {wer(ref, norm(text)):.1%} over {len(ref)} words")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}")
sys.exit(1 if s.failures else 0)
