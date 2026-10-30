"""VRT-29: merge the prototype's ASR words (<p>.asr.json) with its diarization
(<p>.profiles.json) into LocalAI's verbose_json shape, so spikes/localai/analyze.py can score it.

Usage: python3 to_verbose.py <prefix>   -> <prefix>.verbose.json
"""
import json
import sys

p = sys.argv[1]
asr, prof = json.load(open(p + ".asr.json")), json.load(open(p + ".profiles.json"))
diar = prof["segments"]


def speaker(start, end):
    best, best_overlap = None, 0.0
    for d in diar:
        o = min(end, d["end"]) - max(start, d["start"])
        if o > best_overlap:
            best, best_overlap = d["speaker"], o
    if best is None:  # no overlap: nearest diarization segment
        mid = (start + end) / 2
        best = min(diar, key=lambda d: min(abs(mid - d["start"]), abs(mid - d["end"])))["speaker"] if diar else None
    return best


segs = []
for w in asr["words"]:
    spk = speaker(w["start"], w["end"])
    word = {"text": w["w"], "start": w["start"], "end": w["end"]}
    if segs and segs[-1]["speaker"] == spk and w["start"] - segs[-1]["end"] < 1.0:
        segs[-1]["words"].append(word)
        segs[-1]["end"] = w["end"]
    else:
        segs.append({"speaker": spk, "start": w["start"], "end": w["end"], "words": [word]})
for s in segs:
    s["text"] = " ".join(w["text"] for w in s["words"])
json.dump({"text": asr["text"], "segments": segs}, open(p + ".verbose.json", "w"), ensure_ascii=False)
