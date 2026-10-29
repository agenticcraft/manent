"""VRT-30: enroll speakers, then evaluate naming on a test meeting and build a named transcript.

Usage: eval_naming.py <samples/speakers dir> [--enroll enroll] [--meeting test-meeting.wav] [--tag name]
                      [--model parakeet-cpp-bundle-standard] [--url http://127.0.0.1:8181]

Steps:
 1. For each enroll/<Name>.wav: diarize with include_speaker_profiles, register the
    dominant slot under <Name> via portable /v1/voice/register.
 2. Diarize test-meeting.wav (names come from the registry) and transcribe it.
 3. Score naming per reference turn and print cosine distances between the
    test-meeting speaker embeddings and the enrolled ones.
 4. Merge names into the transcription segments: results/named-transcript.txt.
 5. Forget the registered voices again.
"""
import json
import sys
import urllib.request
import uuid
from collections import Counter
from pathlib import Path

import numpy as np

import spikelib
from spikelib import cos_dist, post_file, post_json

args = sys.argv[1:]
base = Path(args[0])
MODEL = args[args.index("--model") + 1] if "--model" in args else "parakeet-cpp-bundle-standard"
spikelib.URL = args[args.index("--url") + 1] if "--url" in args else spikelib.URL
ENROLL = args[args.index("--enroll") + 1] if "--enroll" in args else "enroll"
MEETING = args[args.index("--meeting") + 1] if "--meeting" in args else "test-meeting.wav"
TAG = args[args.index("--tag") + 1] if "--tag" in args else "clean"
results = Path("results")
results.mkdir(exist_ok=True)








# 1. Enroll
enrolled, registered = {}, {}
for wav in sorted((base / ENROLL).glob("*.wav")):
    name = wav.stem
    d = post_file("/v1/audio/diarization", wav, model=MODEL, include_speaker_profiles="true")
    speakers = d["speaker_profiles"]["speakers"]
    best = max(speakers, key=lambda s: s["clean_duration"])
    enrolled[name] = best["embedding"]
    res = post_json("/v1/voice/register", {"model": MODEL, "name": name, "labels": {"source": "vrt-30"},
                                          "speaker_slot": best["speaker"], "speaker_profiles": d["speaker_profiles"]})
    registered[name] = res["id"]
    print(f"enrolled {name}: slot {best['speaker']} ({best['clean_duration']} s clean, {len(speakers)} slot(s) found)")

try:
    # 2. Diarize + transcribe the test meeting
    meeting = base / MEETING
    diar = post_file("/v1/audio/diarization", meeting, model=MODEL, include_speaker_profiles="true")
    trans = post_file("/v1/audio/transcriptions", meeting, model=MODEL, response_format="verbose_json",
                      **{"timestamp_granularities[]": ["word", "segment"]})
    json.dump(diar, open(results / f"naming-{TAG}-diarization.json", "w"))
    json.dump(trans, open(results / f"naming-{TAG}-transcription.json", "w"))
    truth = json.load(open(base / "test-meeting.truth.json"))["segments"]

    def overlap(a0, a1, b0, b1):
        return max(0.0, min(a1, b1) - max(a0, b0))

    # 3a. Naming accuracy per reference turn
    print(f"\ndiarization: {diar.get('num_speakers')} speakers, {len(diar['segments'])} segments")
    tally = Counter()
    print("\nPer turn (reference -> label / name assigned):")
    for t in truth:
        votes = Counter()
        labels = Counter()
        for s in diar["segments"]:
            ov = overlap(t["start"], t["end"], s["start"], s["end"])
            if ov > 0:
                votes[s.get("name")] += ov
                labels[s["label"]] += ov
        got = votes.most_common(1)[0][0] if votes else None
        want = t["name"]
        if want is None:
            verdict = "ok (unnamed)" if got is None else "FALSE NAME"
        elif got == want:
            verdict = "ok"
        elif got is None:
            verdict = "missed"
        else:
            verdict = "WRONG NAME"
        tally[verdict] += 1
        ref = want or f"unknown-{t['speaker_id']}"
        print(f"  [{t['start']:5.1f}-{t['end']:5.1f}] {ref:14} -> label {labels.most_common(1)[0][0] if labels else '-'}  name {got!s:6}  {verdict}")
    print("\nsummary:", dict(tally))

    # 3b. Embedding distances: test-meeting speaker slots vs enrolled voices
    print("\nCosine distance, test-meeting slots vs enrolled voices (lower = more similar):")
    slot_truth = {}
    for sp in diar["speaker_profiles"]["speakers"]:
        # which reference speaker dominates this slot?
        cover = Counter()
        for iv in sp.get("intervals", []):
            for t in truth:
                cover[t["name"] or f"unknown-{t['speaker_id']}"] += overlap(iv["start"], iv["end"], t["start"], t["end"])
        slot_truth[sp["speaker"]] = cover.most_common(1)[0][0] if cover else "?"
    names = sorted(enrolled)
    print("  slot (truth)        " + "  ".join(f"{n:>6}" for n in names))
    same, diff = [], []
    for sp in diar["speaker_profiles"]["speakers"]:
        if not sp.get("embedding"):
            continue
        row = []
        for n in names:
            dist = cos_dist(sp["embedding"], enrolled[n])
            row.append(dist)
            (same if slot_truth[sp["speaker"]] == n else diff).append(dist)
        print(f"  {sp['speaker']} ({slot_truth[sp['speaker']]:14})  " + "  ".join(f"{x:6.3f}" for x in row))
    if same and diff:
        print(f"\nsame speaker: max {max(same):.3f} (mean {np.mean(same):.3f}) | different speaker: min {min(diff):.3f} (mean {np.mean(diff):.3f})")
        print(f"gap between them: {min(diff) - max(same):.3f}; a threshold midway: {(max(same) + min(diff)) / 2:.3f}")

    # 4. Named transcript: each transcription segment gets the name (or label) of the
    #    diarization segment it overlaps most.
    lines = []
    for seg in trans["segments"]:
        best, best_ov = None, 0.0
        for s in diar["segments"]:
            ov = overlap(seg["start"], seg["end"], s["start"], s["end"])
            if ov > best_ov:
                best, best_ov = s, ov
        who = (best.get("name") if best else None) or f"Speaker {int(best['label']) + 1 if best else '?'}"
        if lines and lines[-1][0] == who:
            lines[-1][2] += " " + seg["text"].strip()
        else:
            lines.append([who, seg["start"], seg["text"].strip()])
    text = "\n".join(f"[{int(t // 60):02d}:{t % 60:04.1f}] {who}: {txt}" for who, t, txt in lines)
    (results / f"named-transcript-{TAG}.txt").write_text(text + "\n")
    print(f"\nNamed transcript (results/named-transcript-{TAG}.txt):\n" + text)
finally:
    # 5. Clean up the in-memory registry
    for name, vid in registered.items():
        try:
            post_json("/v1/voice/forget", {"id": vid})
        except urllib.error.HTTPError as e:
            print(f"forget {name}: {e.code}")
