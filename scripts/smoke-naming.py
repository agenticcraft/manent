"""Speaker naming smoke check on the real app (VRT-18). Needs audio hardware and the engine.

Usage: python3 scripts/smoke-naming.py <app binary> <data dir> <shots> <first.wav> <second.wav>
                                       [--names voice=Name,...] [--merge-delete]

tauri-driver must run with MANENT_DATA_DIR=<data dir> (with the engine and model). Both WAVs
need `<wav>.truth.json` (segments with start, end, voice) and are played on the default output
(60% volume; loudness-normalised files). Meeting 1: every voice is named after its truth voice
(or --names), the first through the speaker popover in the UI. With --merge-delete the Speakers
screen is checked too: a duplicate person is merged, and one person (the one with the fewest
words) is deleted. Then the app restarts and meeting 2 is recorded: the people must be
named automatically and correctly, and a deleted person must not get a name.
"""
import json
import sqlite3
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
FIRST, SECOND = Path(sys.argv[4]), Path(sys.argv[5])
NAMES = dict(kv.split("=") for kv in sys.argv[sys.argv.index("--names") + 1].split(",")) if "--names" in sys.argv else {}
MERGE_DELETE = "--merge-delete" in sys.argv


def truth(wav):
    return json.load(open(wav.with_suffix(".truth.json")))["segments"]


def voice_at(segments, t_ms, offset_ms):
    t = (t_ms - offset_ms) / 1000
    voices = {s["voice"] for s in segments if s["start"] <= t <= s["end"]}
    return voices.pop() if len(voices) == 1 else None


def duration(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
                         capture_output=True, text=True, check=True).stdout
    return float(out)


def record(s, wav):
    """Record while playing `wav`; returns (meeting id, play offset ms) once the last rolling pass ran."""
    s.wait_for("return document.querySelector('[data-testid=new-recording]')", 15)
    s.js("document.querySelector('[data-testid=new-recording]').click()")
    s.wait_for("return document.querySelector('[data-testid=recording-timer]') && true", 10)
    session = s.invoke("current_recording")
    time.sleep(1.0)
    play_at = time.time()
    subprocess.run(["paplay", "--volume=39000", str(wav)], check=True)
    time.sleep(50)  # one more rolling pass after the audio ends
    return session["meeting_id"], (play_at - session["started_at"] / 1000) * 1000


def stop_and_finish(s, meeting):
    s.js("document.querySelector('[data-testid=stop]').click()")
    s.wait_for("return document.querySelector('[data-testid=meeting-heading]')", 60)  # Stop opens the recording
    s.js("document.querySelector('[data-testid=back]').click()")
    s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 10)
    deadline = time.time() + 600
    while time.time() < deadline:
        if next(m for m in s.invoke("list_meetings") if m["id"] == meeting)["status"] == "done":
            return True
        time.sleep(1)
    return False


def label_voices(meeting, offset_ms, segments):
    """Truth voice -> the label most of its words got (system track), plus word counts."""
    con = sqlite3.connect(DATA / "manent.db")
    rows = con.execute("""SELECT s.speaker_label, s.words_json FROM segments s JOIN tracks t ON t.id = s.track_id
                          WHERE s.meeting_id = ? AND t.source_kind = 'loopback'""", (meeting,)).fetchall()
    con.close()
    counts = Counter()
    for label, words in rows:
        for w in json.loads(words or "[]"):
            v = voice_at(segments, (w["start_ms"] + w["end_ms"]) / 2, offset_ms)
            if v and label:
                counts[(v, label)] += 1
    best = {}
    for (v, label), n in counts.most_common():
        best.setdefault(v, (label, n))
    return best, counts


def named_words(meeting, offset_ms, segments):
    """(truth voice, person name or None) -> words, for the system track."""
    con = sqlite3.connect(DATA / "manent.db")
    rows = con.execute("""SELECT sp.name, s.words_json FROM segments s JOIN tracks t ON t.id = s.track_id
                          LEFT JOIN speakers sp ON sp.id = s.speaker_id
                          WHERE s.meeting_id = ? AND t.source_kind = 'loopback'""", (meeting,)).fetchall()
    con.close()
    c = Counter()
    for name, words in rows:
        for w in json.loads(words or "[]"):
            v = voice_at(segments, (w["start_ms"] + w["end_ms"]) / 2, offset_ms)
            if v:
                c[(v, name)] += 1
    return c


# ---- Meeting 1: name everyone.
s = Session(APP)
t1 = truth(FIRST)
m1, off1 = record(s, FIRST)
best, _ = label_voices(m1, off1, t1)
print(f"     meeting {m1}: voice -> (label, words) {best}")
names = {v: NAMES.get(v, v) for v in best}
first_voice = max(best, key=lambda v: best[v][1])
# The first one through the UI popover.
label = best[first_voice][0]
s.js(f"document.querySelector('[data-testid=speaker-chip][data-speaker=\"{label}\"]').click()")
s.wait_for("return document.querySelector('[data-testid=speaker-popover]')", 3)
s.screenshot(OUT / "naming-popover.png")
s.js("const i = document.querySelector('[data-testid=speaker-name-input]');"
     "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, arguments[0]);"
     "i.dispatchEvent(new Event('input', {bubbles: true}));", names[first_voice])
s.js("document.querySelector('[data-testid=save-speaker-name]').click()")
shown = s.wait_for(f"const c = document.querySelector('[data-testid=speaker-chip][data-speaker=\"{label}\"]'); return c && c.dataset.person", 5)
s.check(shown == names[first_voice], f"named Speaker {label} '{shown}' from the chip")
for v, (label, _) in best.items():
    if v != first_voice:
        s.invoke("name_speaker", {"meetingId": m1, "label": label, "name": names[v]})
time.sleep(0.5)
s.screenshot(OUT / "naming-named.png")
s.check(stop_and_finish(s, m1), "meeting 1 final pass done")
words1 = named_words(m1, off1, t1)
right = sum(n for (v, name), n in words1.items() if name == names[v])
total = sum(n for (v, name), n in words1.items())
s.check(total and right / total >= 0.9, f"meeting 1 after the final pass: {right}/{total} words carry the given name")

deleted = None
if MERGE_DELETE:
    # A duplicate: another label (or the same one) named as a second person, then merged.
    people_before = {p["name"]: p for p in s.invoke("list_people")}
    dup_voice = first_voice
    s.invoke("name_speaker", {"meetingId": m1, "label": label_voices(m1, off1, t1)[0][dup_voice][0],
                              "name": names[dup_voice] + " copy"})
    s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Speakers')).click()")
    s.wait_for("return document.querySelectorAll('[data-testid=person]').length", 5)
    people = {p["name"]: p for p in s.invoke("list_people")}
    dup, keep = people[names[dup_voice] + " copy"], people[names[dup_voice]]
    s.js("const sel = document.querySelector(`[data-testid=person][data-person-id=\"${arguments[0]}\"] [data-testid=merge-person]`);"
         "Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(sel, String(arguments[1]));"
         "sel.dispatchEvent(new Event('change', {bubbles: true}));", dup["id"], keep["id"])
    time.sleep(0.8)
    after = {p["name"]: p for p in s.invoke("list_people")}
    s.check(len(after) == len(people_before) and after[names[dup_voice]]["samples"] > people_before[names[dup_voice]]["samples"],
            f"duplicate merged on the Speakers screen ({len(after)} people, samples {after[names[dup_voice]]['samples']})")
    # Delete the person with the fewest words.
    deleted_voice = min(best, key=lambda v: best[v][1])
    deleted = names[deleted_voice]
    pid = after[deleted]["id"]
    s.screenshot(OUT / "speakers-screen-dark.png")
    s.js("document.documentElement.classList.remove('dark')")
    time.sleep(0.6)
    s.screenshot(OUT / "speakers-screen-light.png")
    s.js("document.documentElement.classList.add('dark')")
    s.js(f"document.querySelector('[data-testid=person][data-person-id=\"{pid}\"] [data-testid=delete-person]').click()")
    s.wait_for("return document.querySelector('[data-testid=confirm-delete-person]')", 3)
    s.js("document.querySelector('[data-testid=confirm-delete-person]').click()")
    time.sleep(0.8)
    con = sqlite3.connect(DATA / "manent.db")
    left = con.execute("SELECT COUNT(*) FROM voice_samples WHERE speaker_id = ?", (pid,)).fetchone()[0]
    con.close()
    s.check(deleted not in {p["name"] for p in s.invoke("list_people")} and left == 0,
            f"deleted {deleted}: person and voice samples gone ({left} left)")
    s.js("[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('Library')).click()")
s.close()
failures = list(s.failures)

# ---- Restart the app; meeting 2 with the same people.
s = Session(APP)
t2 = truth(SECOND)
m2, off2 = record(s, SECOND)
live = named_words(m2, off2, t2)
print(f"     meeting {m2} live: {dict(live)}")
s.screenshot(OUT / "naming-recognised.png")
s.check(stop_and_finish(s, m2), "meeting 2 final pass done")
words2 = named_words(m2, off2, t2)
print(f"     meeting {m2} final: {dict(words2)}")
known = {v for v in names if names[v] != deleted}
right = sum(n for (v, name), n in words2.items() if v in known and name == names[v])
wrong = sum(n for (v, name), n in words2.items() if name is not None and name != names.get(v))
total = sum(n for (v, name), n in words2.items() if v in known)
s.check(total and right / total >= 0.8, f"after a restart: {right}/{total} words of known people carry their name")
s.check(wrong == 0, f"no words carry a wrong name ({wrong})")
if deleted:
    gone = sum(n for (v, name), n in words2.items() if name == deleted)
    s.check(gone == 0, f"the deleted person is not recognised ({gone} words named {deleted})")
s.close()
failures += s.failures
print(f"\n{'PASS' if not failures else 'FAILED: ' + '; '.join(failures)}; screenshots in {OUT}")
sys.exit(1 if failures else 0)
