"""Import and re-transcribe smoke check on the real app (VRT-19). Needs the engine.

Usage: python3 scripts/smoke-import.py <app binary> <data dir> <screenshot dir> <file> [<file> ...]

tauri-driver must run with VERITAS_DATA_DIR=<data dir> (with the engine and model). Each file
(wav/mp3/m4a/mp4/...) may have `<name>.truth.json` next to it (segments with start, end, text,
and optionally lang and voice). Imports go through the same command as the Import button and
drag and drop (the native dialog itself can't be driven). Checks: jobs show progress and survive
navigation, each import becomes a meeting with speaker blocks, WER per language against the
truth, people named earlier (voice in the truth = the person's name) are recognised, and
Re-transcribe from the meeting screen replaces the transcript.
"""
import json
import re
import sqlite3
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA, OUT = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
FILES = [Path(f).resolve() for f in sys.argv[4:]]


def norm(text):
    return re.sub(r"[^\w\s']", " ", text.lower().replace("-", " ")).split()


def wer(ref, hyp):
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / max(len(ref), 1)


def segments(meeting):
    con = sqlite3.connect(DATA / "veritas.db")
    rows = con.execute("""SELECT s.pass, s.speaker_label, sp.name, s.start_ms, s.end_ms, s.text, s.words_json
                          FROM segments s LEFT JOIN speakers sp ON sp.id = s.speaker_id
                          WHERE s.meeting_id = ? ORDER BY s.start_ms""", (meeting,)).fetchall()
    con.close()
    return rows


def open_page(s, name):
    s.js(f"[...document.querySelectorAll('nav[aria-label=Main] button')].find(b => b.textContent.includes('{name}')).click()")


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=import-audio]')")
known = {p["name"] for p in s.invoke("list_people")}
job_ids = s.invoke("import_audio", {"paths": [str(f) for f in FILES]})
s.check(isinstance(job_ids, list) and len(job_ids) == len(FILES), f"{len(FILES)} imports queued")
MINE = "return [...document.querySelectorAll('[data-testid=job]')].filter(j => arguments[0].includes(Number(j.dataset.jobId))).length"
time.sleep(1)
seen = s.js(MINE, job_ids)
s.check(seen == len(FILES), f"jobs listed in the Library ({seen})")
progressing = s.wait_for("const j = document.querySelector('[data-testid=job][data-stage=transcribing]'); return j && j.textContent", 60)
s.check(bool(progressing) and "%" in progressing, f"progress shown: {progressing!r}")
s.screenshot(OUT / "import-jobs.png")
# Navigate away and back: the jobs are still there.
open_page(s, "Settings")
time.sleep(1)
open_page(s, "Library")
time.sleep(1)
still = s.js(MINE, job_ids)
s.check(still == len(FILES), f"jobs survive navigation ({still})")

start = time.time()
jobs = []
while time.time() - start < 900:
    jobs = s.invoke("list_jobs")
    mine = [j for j in jobs if j["id"] in job_ids]
    if all(j["stage"] in ("done", "failed") for j in mine):
        break
    time.sleep(1)
mine = {j["id"]: j for j in jobs if j["id"] in job_ids}
s.check(all(j["stage"] == "done" for j in mine.values()),
        f"all imports done in {time.time() - start:.0f} s ({[(j['name'], j['stage'], j['error']) for j in mine.values()]})")
s.screenshot(OUT / "import-done.png")

meetings = {m["id"]: m for m in s.invoke("list_meetings")}
for f, jid in zip(FILES, job_ids):
    job = mine[jid]
    mid = job["meeting_id"]
    rows = segments(mid)
    labels = Counter(r[1] for r in rows)
    s.check(meetings[mid]["status"] == "done" and rows and set(r[0] for r in rows) == {"final"},
            f"{f.name}: meeting '{meetings[mid]['title']}' with {len(rows)} final speaker blocks {dict(labels)}")
    truth_path = f.with_name(f.stem + ".truth.json")
    if not truth_path.exists():
        continue
    truth = json.load(open(truth_path))["segments"]
    hyp_words = [w for r in rows for w in json.loads(r[6] or "[]")]
    if all("text" in t for t in truth):
        by_lang = {}
        for t in truth:
            a, b = t["start"] * 1000 - 300, t["end"] * 1000 + 300
            hyp = " ".join(w["text"] for w in hyp_words if a <= w["start_ms"] <= b)
            ref = norm(t["text"])
            e = by_lang.setdefault(t.get("lang", "?"), [0.0, 0])
            e[0] += wer(ref, norm(hyp)) * len(ref)
            e[1] += len(ref)
        for lang, (errs, n) in by_lang.items():
            s.check(errs / n <= 0.2, f"{f.name}: {lang} WER {errs / n:.1%}")
    # Known people: words of a truth voice that is a known person's name carry that name.
    if any(t.get("voice") in known for t in truth):
        tally = Counter()
        for r in rows:
            for w in json.loads(r[6] or "[]"):
                mid_s = (w["start_ms"] + w["end_ms"]) / 2000
                voices = {t["voice"] for t in truth if t["start"] <= mid_s <= t["end"]}
                if len(voices) == 1 and (v := voices.pop()) in known:
                    tally[(v, r[2])] += 1
        right = sum(n for (v, name), n in tally.items() if name == v)
        total = sum(tally.values())
        s.check(total and right / total >= 0.8, f"{f.name}: known people recognised ({right}/{total} words): {dict(tally)}")

# Re-transcribe the first import from its meeting screen.
first = mine[job_ids[0]]["meeting_id"]
before = segments(first)
s.js("document.querySelector('[data-testid=open-meeting]') && 0")
s.js(f"[...document.querySelectorAll('[data-testid=meeting-row]')].find(r => r.textContent.includes(arguments[0])).querySelector('[data-testid=open-meeting]').click()",
     meetings[first]["title"])
s.wait_for("return document.querySelector('[data-testid=meeting-transcript]')", 10)
blocks = s.js("return document.querySelectorAll('[data-testid=caption-block]').length")
s.check(blocks > 0, f"meeting screen shows the transcript ({blocks} blocks)")
s.screenshot(OUT / "meeting-transcript-dark.png")
s.js("document.documentElement.classList.remove('dark')")
time.sleep(0.6)
s.screenshot(OUT / "meeting-transcript-light.png")
s.js("document.documentElement.classList.add('dark')")
s.js("document.querySelector('[data-testid=retranscribe]').click()")
running = s.wait_for("const j = document.querySelector('[data-testid=meeting-job]'); return j && j.dataset.stage", 10)
s.check(bool(running), f"re-transcribe shows its job on the meeting screen ({running})")
gone = s.wait_for("return !document.querySelector('[data-testid=meeting-job]') && document.querySelectorAll('[data-testid=caption-block]').length", 600)
after = segments(first)
s.check(bool(gone) and after and len(after) > 0 and set(r[0] for r in after) == {"final"},
        f"re-transcribe replaced the transcript ({len(before)} -> {len(after)} blocks)")
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}; screenshots in {OUT}")
sys.exit(1 if s.failures else 0)
