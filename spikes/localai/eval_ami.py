"""VRT-31: offline evaluation of LocalAI parakeet on an AMI meeting.

Usage: eval_ami.py <meeting.wav> <meeting.reference.json> [--model parakeet-cpp-bundle-standard] [--url http://127.0.0.1:8181]

Runs /v1/audio/transcriptions and /v1/audio/diarization (with speaker profiles),
saves both to results/<meeting>-*.json and prints:
  - RTF for both calls
  - WER against the AMI words (fillers removed on both sides)
  - diarization: number of speakers, and per 10 ms frame of single-speaker reference
    speech whether the detected speaker maps to the right person
"""
import json
import re
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np

import spikelib
from spikelib import post_file, wer

FILLERS = {"um", "uh", "hmm", "mm", "mm-hmm", "mmhmm", "uh-huh", "huh", "ah", "oh", "er", "erm", "hm", "mhm", "eh"}
FRAME = 0.01


def norm(text):
    words = re.sub(r"[^\w\s'-]", " ", text.lower()).replace("-", " ").split()
    return [w for w in words if w not in FILLERS and w.replace(" ", "") not in FILLERS]


def frames(intervals, n):
    """Boolean frame mask for a list of (start, end) seconds."""
    mask = np.zeros(n, dtype=bool)
    for s, e in intervals:
        mask[int(s / FRAME):int(e / FRAME) + 1] = True
    return mask


def main():
    args = sys.argv[1:]
    wav, ref_path = Path(args[0]), args[1]
    ref = json.load(open(ref_path))
    meeting = ref["meeting"]
    model = args[args.index("--model") + 1] if "--model" in args else "parakeet-cpp-bundle-standard"
    spikelib.URL = args[args.index("--url") + 1] if "--url" in args else spikelib.URL
    out = Path("results")
    out.mkdir(exist_ok=True)
    duration = max(w["end"] for w in ref["words"]) + 1

    t0 = time.time()
    trans = post_file("/v1/audio/transcriptions", wav, model=model, response_format="verbose_json",
                      **{"timestamp_granularities[]": ["word", "segment"]})
    t_trans = time.time() - t0
    t0 = time.time()
    diar = post_file("/v1/audio/diarization", wav, model=model, include_speaker_profiles="true")
    t_diar = time.time() - t0
    json.dump(trans, open(out / f"{meeting}-transcription.json", "w"))
    json.dump(diar, open(out / f"{meeting}-diarization.json", "w"))
    audio_s = diar.get("duration") or duration
    print(f"{meeting}: {audio_s / 60:.1f} min audio | transcription {t_trans:.0f}s (RTF {t_trans / audio_s:.3f}) | "
          f"diarization {t_diar:.0f}s (RTF {t_diar / audio_s:.3f})")

    # WER
    hyp_words = norm(" ".join(s["text"] for s in trans["segments"]))
    ref_words = norm(" ".join(w["word"] for w in ref["words"]))
    print(f"WER {wer(ref_words, hyp_words):.1%}  (ref {len(ref_words)} words, hyp {len(hyp_words)} words, fillers removed)")

    # Diarization: reference speech from word timings, single-speaker frames only
    n = int(duration / FRAME) + 2
    ref_masks = {spk: frames([(w["start"], w["end"]) for w in ref["words"] if w["speaker"] == spk], n)
                 for spk in sorted(set(ref["speakers"].values()))}
    stack = np.stack(list(ref_masks.values()))
    single = stack.sum(axis=0) == 1
    hyp_masks = defaultdict(lambda: np.zeros(n, dtype=bool))
    for s in diar["segments"]:
        hyp_masks[s["label"]] |= frames([(s["start"], s["end"])], n)
    mapping = {}
    for label, hm in hyp_masks.items():
        ov = {spk: int((hm & rm & single).sum()) for spk, rm in ref_masks.items()}
        mapping[label] = max(ov, key=ov.get)
    correct = missed = confused = 0
    per_spk = {}
    for spk, rm in ref_masks.items():
        frames_spk = rm & single
        got = np.zeros(n, dtype=bool)
        any_hyp = np.zeros(n, dtype=bool)
        for label, hm in hyp_masks.items():
            any_hyp |= hm
            if mapping[label] == spk:
                got |= hm
        c = int((frames_spk & got).sum())
        m = int((frames_spk & ~any_hyp).sum())
        correct += c
        missed += m
        confused += int(frames_spk.sum()) - c - m
        per_spk[spk] = (c, int(frames_spk.sum()))
    total = correct + missed + confused
    print(f"diarization: {len(hyp_masks)} speakers found (reference {len(ref_masks)}); label -> person: {mapping}")
    print(f"single-speaker speech attributed to the right person {correct / total:.1%}, "
          f"wrong person {confused / total:.1%}, missed {missed / total:.1%}  "
          f"(overlap = {(stack.sum(axis=0) > 1).sum() * FRAME / 60:.1f} min excluded)")
    for spk, (c, t) in per_spk.items():
        print(f"  {spk}: {c / max(t, 1):.1%} of {t * FRAME / 60:.1f} min")
    # Profiles with the person each slot maps to, for the cross-session naming test
    profiles = []
    for sp in diar["speaker_profiles"]["speakers"]:
        profiles.append({"slot": sp["speaker"], "person": mapping.get(str(sp["speaker"])),
                         "clean_duration": sp["clean_duration"], "embedding": sp.get("embedding")})
        print(f"  profile slot {sp['speaker']} -> {mapping.get(str(sp['speaker']))}, clean {sp['clean_duration']} s")
    json.dump({"meeting": meeting, "encoder": diar["speaker_profiles"]["encoder"], "profiles": profiles},
              open(out / f"{meeting}-profiles.json", "w"))


if __name__ == "__main__":
    main()
