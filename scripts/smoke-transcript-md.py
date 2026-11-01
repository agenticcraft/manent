"""transcript.md next to each meeting's audio, on the real app (VRT-37).

Usage: python3 scripts/smoke-transcript-md.py <app binary> <data dir>

tauri-driver must run with VERITAS_DATA_DIR=<data dir>, a folder with transcribed meetings
(e.g. after smoke-import.py). Removes the files of some meetings first, so the app writes them
again at startup (the backfill). Checks each file: a YAML header that a YAML parser reads, the
same blocks as the app shows, the audio files that exist; then that renaming the meeting,
naming a speaker and changing the user's own name rewrite it.
"""
import re
import sqlite3
import sys
import time
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).parent))
from wdlib import Session  # noqa: E402

APP, DATA = sys.argv[1], Path(sys.argv[2])
FILE = "transcript.md"

con = sqlite3.connect(DATA / "veritas.db")
done = con.execute("""SELECT m.id, m.dir FROM meetings m WHERE m.status = 'done'
                      AND EXISTS (SELECT 1 FROM segments s WHERE s.meeting_id = m.id AND s.text != '')
                      ORDER BY m.id""").fetchall()
con.close()
for _, d in done:
    (Path(d) / FILE).unlink(missing_ok=True)


def read(d):
    text = (Path(d) / FILE).read_text()
    _, head, body = text.split("---\n", 2)
    blocks = re.findall(r"^\*\*(.+)\*\* · ([\d:]+)\n\n(.+)$", body, re.M)
    return yaml.safe_load(head), blocks, text


def wait_file(d, check, timeout=10):
    end = time.time() + timeout
    while time.time() < end:
        try:
            got = read(d)
            if check(got):
                return got
        except (OSError, ValueError, yaml.YAMLError):
            pass
        time.sleep(0.2)
    return None


s = Session(APP)
s.wait_for("return document.querySelector('[data-testid=meeting-row]')", 15)
written = [m for m, d in done if wait_file(d, lambda g: True)]
s.check(len(written) == len(done), f"backfill wrote {len(written)} of {len(done)} meetings' {FILE}")

# Contents of the longest one against what the app shows.
mid, mdir = max(done, key=lambda m: len(s.invoke("live_transcript", {"meetingId": m[0]})))
head, blocks, text = read(mdir)
meeting = s.invoke("get_meeting", {"id": mid})
s.check(head["veritas"] == 1 and head["id"] == mid and head["duration_ms"] == meeting["duration_ms"]
        and head["pass"] == "final", f"header: {dict((k, head[k]) for k in ['veritas', 'id', 'title', 'duration_ms', 'pass'])}")
s.check(head["audio"] and all((Path(mdir) / a).is_file() for a in head["audio"]), f"audio files exist: {head['audio']}")
words_app = sum(len(x["text"].split()) for x in s.invoke("live_transcript", {"meetingId": mid}))
words_md = sum(len(b[2].split()) for b in blocks)
s.check(blocks and words_md == words_app, f"{len(blocks)} blocks, {words_md} words (app {words_app})")
s.check(set(head["speakers"]) == {b[0] for b in blocks}, f"speakers {head['speakers']}")

# Changes rewrite it.
s.invoke("rename_meeting", {"id": mid, "title": "Smoke: renamed"})
got = wait_file(mdir, lambda g: g[0]["title"] == "Smoke: renamed" and "# Smoke: renamed" in g[2])
s.check(bool(got), "renaming the meeting rewrites it")
label = next((v["label"] for v in s.invoke("meeting_voices", {"meetingId": mid})), None)
if label:
    s.invoke("name_speaker", {"meetingId": mid, "label": label, "name": "Smoke Person"})
    got = wait_file(mdir, lambda g: "Smoke Person" in g[0]["speakers"] and any(b[0] == "Smoke Person" for b in g[1]))
    s.check(bool(got), f"naming speaker {label} rewrites it")
mic_meeting = next((m for m in done if any(b[0] == s.invoke("get_me_name") for b in read(m[1])[1])), None)
if mic_meeting:
    before = s.invoke("get_me_name")
    s.invoke("set_me_name", {"name": "Smoke Me"})
    got = wait_file(mic_meeting[1], lambda g: "Smoke Me" in g[0]["speakers"])
    s.check(bool(got), "changing the user's own name rewrites it")
    s.invoke("set_me_name", {"name": before})
s.close()
print(f"\n{'PASS' if not s.failures else 'FAILED: ' + '; '.join(s.failures)}")
sys.exit(1 if s.failures else 0)
