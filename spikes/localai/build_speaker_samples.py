"""VRT-30: build speaker-naming test data from LibriSpeech test-clean (CC-BY-4.0).

Usage: build_speaker_samples.py <out_dir>

Writes:
  enroll/<Name>.wav   ~20 s of one enrolled speaker (enrollment utterances)
  test-meeting.wav    turns from *different* utterances of the enrolled
                      speakers plus speakers who were never enrolled
  test-meeting.truth.json
"""
import json
import subprocess
import sys
import time
import urllib.request
import wave
from pathlib import Path

import numpy as np

API = "https://datasets-server.huggingface.co/rows?dataset=openslr/librispeech_asr&config=clean&split=test&offset={off}&length=100"
ENROLLED = {2094: "Anna", 8230: "Ben", 4992: "Clara", 2300: "David"}
UNKNOWN = [672, 3570]
# Row windows that contain each speaker (LibriSpeech rows are grouped by speaker).
WINDOWS = {2094: [300, 400], 8230: [600, 700], 4992: [2100, 2200], 2300: [1800, 1900], 672: [1500, 1600], 3570: [2400, 2500]}
SR = 16000
ORDER = ["Anna", "Ben", 672, "Clara", "David", "Anna", 3570, "Ben", "Clara", "David"]


def get(url, tries=4):
    req = urllib.request.Request(url, headers={"User-Agent": "curl/8.0 veritas-spike"})
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001 - transient network errors
            if attempt == tries - 1:
                raise
            print(f"  retry {attempt + 1}: {e}")
            time.sleep(2 * (attempt + 1))


def decode(flac_bytes):
    """FLAC bytes -> 16 kHz mono float32 via ffmpeg."""
    pcm = subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-i", "pipe:0", "-ac", "1", "-ar", str(SR), "-f", "s16le", "pipe:1"],
        input=flac_bytes, capture_output=True, check=True).stdout
    return np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768


def write_wav(path, x):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((np.clip(x, -1, 1) * 32767).astype("<i2").tobytes())


def rows_for(spk):
    rows = []
    for off in WINDOWS[spk]:
        rows += [r["row"] for r in json.loads(get(API.format(off=off)))["rows"] if r["row"]["speaker_id"] == spk]
    return rows


def audio_of(row):
    a = row["audio"][0] if isinstance(row["audio"], list) else row["audio"]
    return decode(get(a["src"]))


out = Path(sys.argv[1])
(out / "enroll").mkdir(parents=True, exist_ok=True)
test_clips = {}  # key (name or speaker id) -> list of (audio, row)
enroll_info = {}

for spk in list(ENROLLED) + UNKNOWN:
    rows = rows_for(spk)
    chapters = sorted({r["chapter_id"] for r in rows})
    print(f"speaker {spk}: {len(rows)} rows, chapters {chapters}")
    key = ENROLLED.get(spk, spk)
    if spk in ENROLLED:
        # Enrollment: utterances from the first chapter until >= 20 s.
        enroll_rows, enroll_audio, total = [], [], 0.0
        for r in [r for r in rows if r["chapter_id"] == chapters[0]]:
            x = audio_of(r)
            enroll_rows.append(r["id"]); enroll_audio += [x, np.zeros(int(0.3 * SR), np.float32)]
            total += len(x) / SR
            if total >= 20:
                break
        write_wav(out / "enroll" / f"{key}.wav", np.concatenate(enroll_audio))
        enroll_info[key] = {"speaker_id": spk, "utterances": enroll_rows, "seconds": round(total, 1)}
        # Test: utterances 5-15 s from another chapter if there is one, else later rows of the same chapter.
        pool = [r for r in rows if r["chapter_id"] != chapters[0]] or [r for r in rows if r["id"] not in enroll_rows]
    else:
        pool = rows
    picked = []
    for r in pool:
        if r["id"] in enroll_info.get(key, {}).get("utterances", []):
            continue
        x = audio_of(r)
        if 5 <= len(x) / SR <= 15:
            picked.append((x, r))
        if len(picked) == 2:
            break
    test_clips[key] = picked

parts, segs, t, used = [np.zeros(int(0.5 * SR), np.float32)], [], 0.5, {}
for key in ORDER:
    i = used.get(key, 0)
    used[key] = i + 1
    x, r = test_clips[key][i % len(test_clips[key])]
    dur = len(x) / SR
    segs.append({"start": round(t, 2), "end": round(t + dur, 2), "speaker_id": r["speaker_id"],
                 "name": key if isinstance(key, str) else None, "enrolled": isinstance(key, str),
                 "chapter_id": r["chapter_id"], "utterance": r["id"], "text": r["text"].lower()})
    parts += [x, np.zeros(int(0.8 * SR), np.float32)]
    t += dur + 0.8
write_wav(out / "test-meeting.wav", np.concatenate(parts))
json.dump({"source": "openslr/librispeech_asr clean/test (CC-BY-4.0)", "enroll": enroll_info, "segments": segs},
          open(out / "test-meeting.truth.json", "w"), indent=2)
print(f"\ntest-meeting.wav: {t:.1f}s, {len(segs)} turns")
for s in segs:
    print(f"  {s['start']:5.1f}-{s['end']:5.1f} {str(s['name'] or 'unknown-' + str(s['speaker_id'])):14} ch{s['chapter_id']}  {s['text'][:60]}")
for k, v in enroll_info.items():
    print(f"enroll {k}: {v['seconds']} s from {len(v['utterances'])} utterances")
