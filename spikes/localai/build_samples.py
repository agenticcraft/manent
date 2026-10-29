"""Download FLEURS (nl_nl test, en_us validation) and CS-FLEURS nld-eng clips and
build a fake bilingual meeting with ground truth. Usage: build_samples.py <out_dir>"""
import io
import json
import sys
import time
import urllib.request
import wave

import numpy as np
from pathlib import Path

from wavio import read_wav

out = Path(sys.argv[1])
(out / "fleurs").mkdir(parents=True, exist_ok=True)
(out / "cs-fleurs").mkdir(parents=True, exist_ok=True)
API = "https://datasets-server.huggingface.co/rows?dataset={ds}&config={cfg}&split={split}&offset={off}&length=100"


def get(url, tries=4):
    req = urllib.request.Request(url, headers={"User-Agent": "curl/8.0 veritas-spike"})
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001 - transient network errors
            if attempt == tries - 1:
                raise
            print(f"  retry {attempt + 1} for {url[:80]}: {e}")
            time.sleep(2 * (attempt + 1))


def rows(ds, cfg, split, pages=3):
    res = []
    for off in range(0, pages * 100, 100):
        res += [r["row"] for r in json.loads(get(API.format(ds=ds, cfg=cfg, split=split, off=off)))["rows"]]
    return res


def median_f0(data):
    """Median pitch (Hz) of voiced 40 ms frames, via autocorrelation."""
    sr, x = read_wav(data)
    f0s, n = [], int(0.04 * sr)
    for i in range(0, len(x) - n, n):
        fr = x[i:i + n] - x[i:i + n].mean()
        if np.sqrt((fr ** 2).mean()) < 0.01:
            continue
        ac = np.correlate(fr, fr, "full")[n - 1:]
        lo, hi = sr // 400, sr // 70
        lag = lo + int(np.argmax(ac[lo:hi]))
        if ac[lag] > 0.3 * ac[0]:
            f0s.append(sr / lag)
    return float(np.median(f0s)) if f0s else 0.0


def low_high(rs, n=2, candidates=24):
    """Download candidate clips (4-12 s) and return n low-pitched and n high-pitched ones."""
    scored = []
    for r in rs:
        if not 4 <= r["num_samples"] / 16000 <= 12:
            continue
        audio = r["audio"][0] if isinstance(r["audio"], list) else r["audio"]
        r["_wav"] = get(audio["src"])
        r["_f0"] = median_f0(r["_wav"])
        if r["_f0"]:
            scored.append(r)
        if len(scored) == candidates:
            break
    scored.sort(key=lambda r: r["_f0"])
    print("  f0 range:", [round(r["_f0"]) for r in scored])
    return scored[:n], scored[-n:]


nl, en = rows("google/fleurs", "nl_nl", "test"), rows("google/fleurs", "en_us", "validation")
nl_low, nl_high = low_high(nl)
en_low, en_high = low_high(en)
voices = {"nl-f": nl_high, "en-m": en_low, "nl-m": nl_low, "en-f": en_high}
order = ["nl-f", "en-m", "nl-m", "en-f", "nl-f", "en-m", "nl-m", "en-f"]

parts, truth, t, rate = [], [], 0.0, None
gap = 0.8
for i, label in enumerate(order):
    r = voices[label][i // 4 % len(voices[label])]
    data = r["_wav"]
    clip = out / "fleurs" / f"{i:02d}-{label}-{r['id']}.wav"
    clip.write_bytes(data)
    sr, x = read_wav(data)
    rate = rate or sr
    assert sr == rate, f"rate mismatch {sr} vs {rate}"
    dur = len(x) / sr
    parts += [x, np.zeros(int(gap * sr), dtype=np.float32)]
    truth.append({"start": round(t, 2), "end": round(t + dur, 2), "voice": label,
                  "lang": label[:2], "pitch_hz": round(r["_f0"]), "fleurs_id": r["id"], "text": r["transcription"]})
    t += dur + gap

pcm16 = (np.clip(np.concatenate(parts), -1, 1) * 32767).astype("<i2").tobytes()
with wave.open(str(out / "meeting-nl-en.wav"), "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate)
    w.writeframes(pcm16)
params = (1, 2, rate)
(out / "meeting-nl-en.truth.json").write_text(json.dumps(
    {"source": "google/fleurs (CC-BY-4.0): nl_nl test, en_us validation",
     "note": "FLEURS has no speaker ids and its gender labels are unreliable (all nl_nl rows are 0). Voices are picked by measured pitch: '-m' = lowest-pitched clips, '-f' = highest-pitched clips per language. Two clips with the same voice label may still be different people.",
     "format": {"channels": params[0], "sample_width": params[1], "rate": params[2]},
     "segments": truth}, indent=2, ensure_ascii=False))
print(f"meeting-nl-en.wav: {t:.1f}s, {len(truth)} turns, format {params}")

# CS-FLEURS: intra-sentence Dutch/English code-switching (synthetic MMS TTS, CC-BY-NC-4.0).
base = "https://huggingface.co/datasets/byan/cs-fleurs/resolve/main/mms/test"
want = {f"concat.{i}.wav" for i in range(3)}
meta = []
for line in get(f"{base}/metadata.jsonl").decode().splitlines():
    m = json.loads(line)
    if "nld_eng" in m.get("file_name", "") and m["file_name"].split("/")[-1] in want:
        meta.append(m)
for m in meta:
    name = m["file_name"].split("/")[-1]
    (out / "cs-fleurs" / name).write_bytes(get(f"{base}/{m['file_name']}"))
(out / "cs-fleurs" / "truth.json").write_text(json.dumps(
    {"source": "byan/cs-fleurs mms/test nld-eng (CC-BY-NC-4.0, synthetic TTS)", "clips": meta}, indent=2, ensure_ascii=False))
print(f"cs-fleurs: {len(meta)} clips")
for m in meta:
    print("  ", m["file_name"].split("/")[-1], "|", str(m.get("text", ""))[:120])
