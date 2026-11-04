"""Speaker labels smoke check on the real app (VRT-17). Needs audio hardware and the engine.

Usage: python3 scripts/smoke-speakers.py <app binary> <data dir> <screenshot dir> <speech.wav> [--rename]

tauri-driver must run with MANENT_DATA_DIR=<data dir>, a temp folder with the engine and model
downloaded. The WAV (multi-speaker; `<speech>.truth.json` next to it with segments
{start, end, voice}) is played on the default output (60% volume; use a loudness-normalised
file) during a recording. Checks:
- rolling passes replace the captions with speaker blocks while recording, within ~1 min;
- each truth voice keeps one label, and different voices get different labels (rolling and
  final pass);
- the final pass runs after Stop (meeting 'processing' -> 'done') and replaces everything;
- mic bleed: few "Me" words repeat the system track;
- with --rename: "Me" can be renamed from its chip.
"""
import json
import re
import sqlite3
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT, SPEECH = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4])
RENAME = "--rename" in sys.argv
TRUTH = json.load(open(SPEECH.with_suffix(".truth.json")))["segments"]


def duration(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
                         capture_output=True, text=True, check=True).stdout
    return float(out)


def norm(t):
    return re.sub(r"\W", "", t.lower())


def voice_at(t_ms, offset_ms):
    """The one truth voice speaking at this track time, or None (silence or overlap)."""
    t = (t_ms - offset_ms) / 1000
    voices = {seg["voice"] for seg in TRUTH if seg["start"] <= t <= seg["end"]}
    return voices.pop() if len(voices) == 1 else None


def consistency(segments, offset_ms, kinds):
    """Word level: every system-track word where exactly one truth voice speaks counts; each
    voice's main label is the label most of its words got. Returns (voice -> label, share of
    words with their voice's main label, words counted)."""
    pairs = [(voice_at((w["start_ms"] + w["end_ms"]) / 2, offset_ms), seg["label"])
             for seg in segments if kinds.get(seg["track_id"]) == "loopback" and seg["label"]
             for w in seg["words"]]
    pairs = [(v, label) for v, label in pairs if v]
    main = {v: Counter(label for vv, label in pairs if vv == v).most_common(1)[0][0] for v in {v for v, _ in pairs}}
    agree = sum(1 for v, label in pairs if main[v] == label)
    return main, agree / max(len(pairs), 1), len(pairs)


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=new-recording]')")
status = s.invoke("engine_status")
if not (status["engine_installed"] and status["model_installed"]):
    sys.exit("engine/model not downloaded in this data folder (run smoke-engine.py first)")

s.js("document.querySelector('[data-testid=new-recording]').click()")
s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
session = s.invoke("current_recording")
meeting, started = session["meeting_id"], session["started_at"] / 1000
time.sleep(1.0)
play_at = time.time()
offset_ms = (play_at - started) * 1000
length = duration(SPEECH)
player = subprocess.Popen(["paplay", "--volume=39000", str(SPEECH)])

# Watch the rolling passes replace captions: for each truth turn, when it first got labels.
labelled_at = {}
snapshot = []
end = play_at + length + 50  # one more rolling pass after the audio ends
while time.time() < end:
    snapshot = s.invoke("live_transcript", {"meetingId": meeting})
    rolling = [seg for seg in snapshot if seg["pass"] == "rolling"]
    for i, t in enumerate(TRUTH):
        if t["end"] - t["start"] < 1.5:
            continue  # backchannels often have no words
        if i not in labelled_at and any(seg["start_ms"] <= offset_ms + t["end"] * 1000 and seg["end_ms"] >= offset_ms + t["start"] * 1000
                                        for seg in rolling):
            labelled_at[i] = time.time() - (play_at + t["end"])
    time.sleep(0.5)
player.wait()

con = sqlite3.connect(DATA / "manent.db")
kinds = dict(con.execute("SELECT id, source_kind FROM tracks WHERE meeting_id = ?", (meeting,)).fetchall())
con.close()
waits = sorted(labelled_at.values())
turns = sum(1 for t in TRUTH if t["end"] - t["start"] >= 1.5)
s.check(len(labelled_at) >= turns * 0.9 and waits and waits[len(waits) // 2] <= 60,
        f"rolling passes labelled {len(labelled_at)}/{turns} turns of 1.5 s or more, median {waits[len(waits) // 2] if waits else 0:.0f} s"
        f" and max {waits[-1] if waits else 0:.0f} s after the turn ended")
main, share, n = consistency(snapshot, offset_ms, kinds)
print(f"     rolling: voice -> label {main}")
s.check(n and share >= 0.9,
        f"rolling: {share:.0%} of {n} words carry their voice's label; {len(set(main.values()))} labels for {len(main)} voices")
chips = s.js("return [...document.querySelectorAll('[data-testid=speaker-chip]')].map(c => c.dataset.speaker)")
current = s.js("return document.querySelectorAll('[data-testid=caption-block][data-current=true]').length")
s.check(len(set(chips) - {"me"}) >= 2 and current == 1, f"speaker chips shown ({sorted(set(chips))}), one current block")
s.screenshot(OUT / "speakers-live-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "speakers-live-light.png")
s.js("document.documentElement.classList.add('dark')")

if RENAME:
    has_me = s.js("return !!document.querySelector('[data-testid=speaker-chip][data-speaker=me]')")
    if has_me:
        s.js("document.querySelector('[data-testid=speaker-chip][data-speaker=me]').click()")
        s.wait_for("return document.querySelector('[data-testid=me-name-input]')", 3)
        s.js("const i = document.querySelector('[data-testid=me-name-input]');"
             "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'Rob');"
             "i.dispatchEvent(new Event('input', {bubbles: true})); i.blur();")
        shown = s.wait_for("const c = document.querySelector('[data-testid=speaker-chip][data-speaker=me]'); return c && c.textContent", 3)
        stored = s.invoke("get_me_name")
        s.check(shown == "Rob" and stored == "Rob", f"'Me' renamed from its chip (shown {shown!r}, stored {stored!r})")
        s.invoke("set_me_name", {"name": "Me"})
    else:
        print("     (no 'Me' block in view to rename)")

stop_at = time.time()
s.js("document.querySelector('[data-testid=stop]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 60)  # Stop opens the recording
s.js("document.querySelector('[data-testid=back]').click()")
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 10)
processing = s.invoke("list_meetings")
state = next(m["status"] for m in processing if m["id"] == meeting)
s.check(state in ("processing", "done"), f"meeting is '{state}' right after Stop")
done = None
deadline = time.time() + 600
while time.time() < deadline:
    m = next(m for m in s.invoke("list_meetings") if m["id"] == meeting)
    if m["status"] == "done":
        done = time.time() - stop_at
        break
    time.sleep(1)
s.check(done is not None, f"final pass finished {done and round(done)} s after Stop ({length:.0f} s of audio)")
s.close()

con = sqlite3.connect(DATA / "manent.db")
rows = con.execute("SELECT track_id, pass, speaker_label, start_ms, end_ms, text, words_json FROM segments WHERE meeting_id = ?",
                   (meeting,)).fetchall()
con.close()
passes = Counter(r[1] for r in rows)
s.check(set(passes) == {"final"}, f"only final segments remain ({dict(passes)})")
final = [{"track_id": r[0], "label": r[2], "start_ms": r[3], "end_ms": r[4], "words": json.loads(r[6] or "[]")} for r in rows]
main, share, n = consistency(final, offset_ms, kinds)
print(f"     final: voice -> label {main}")
s.check(n and share >= 0.95 and len(set(main.values())) >= 2,
        f"final: {share:.0%} of {n} words carry their voice's label; {len(set(main.values()))} labels for {len(main)} voices")
system_words = [w for f in final if kinds[f["track_id"]] == "loopback" for w in f["words"]]
me_words = [w for f in final if f["label"] == "me" for w in f["words"]]
echoed = sum(1 for w in me_words if any(norm(w["text"]) == norm(x["text"]) and abs(w["start_ms"] - x["start_ms"]) <= 1000
                                       for x in system_words))
print(f"     'Me' words {len(me_words)}, of which {echoed} repeat the system track")
s.check(not me_words or echoed / len(me_words) <= 0.15, f"bleed removed from 'Me' ({echoed}/{len(me_words)} echoed)")

print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
