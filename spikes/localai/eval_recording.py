"""VRT-32: offline run on a recording without exact ground truth.

Usage: eval_recording.py <audio.flac> [--captions <file.vtt>] [--language nl] [--model parakeet-cpp-bundle-standard]

Saves results/<name>-transcription.json, -diarization.json and -profiles.json and prints
RTF, speakers (talk time, clean seconds), and - with captions - the word disagreement
with the YouTube auto-captions (rough reference: they are ASR output themselves).
"""
import json
import re
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

import spikelib
from eval_ami import norm
from spikelib import post_file, wer


def vtt_text(path):
    """Plain text from a YouTube auto-caption VTT (rolling captions de-duplicated)."""
    lines, last = [], None
    for line in open(path, encoding="utf-8"):
        line = re.sub(r"<[^>]+>", "", line).strip()
        if not line or "-->" in line or line.startswith(("WEBVTT", "Kind:", "Language:")) or line == last:
            continue
        lines.append(line)
        last = line
    # rolling captions repeat the previous line; keep a line only if it isn't the start of the next
    out = [l for i, l in enumerate(lines) if i + 1 == len(lines) or not lines[i + 1].startswith(l)]
    return re.sub(r"\[[^\]]*\]", " ", " ".join(out))


def main():
    args = sys.argv[1:]
    audio = Path(args[0])
    name = audio.stem
    model = args[args.index("--model") + 1] if "--model" in args else "parakeet-cpp-bundle-standard"
    language = args[args.index("--language") + 1] if "--language" in args else None
    captions = args[args.index("--captions") + 1] if "--captions" in args else None
    out = Path("results")
    out.mkdir(exist_ok=True)
    duration = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(audio)],
                                    capture_output=True, text=True, check=True).stdout)
    fields = {"model": model, "response_format": "verbose_json", "timestamp_granularities[]": ["word", "segment"]}
    if language:
        fields["language"] = language
    t0 = time.time()
    trans = post_file("/v1/audio/transcriptions", audio, **fields)
    t_trans = time.time() - t0
    t0 = time.time()
    diar = post_file("/v1/audio/diarization", audio, model=model, include_speaker_profiles="true")
    t_diar = time.time() - t0
    json.dump(trans, open(out / f"{name}-transcription.json", "w"))
    json.dump(diar, open(out / f"{name}-diarization.json", "w"))
    print(f"{name}: {duration / 60:.1f} min | transcription {t_trans:.0f}s (RTF {t_trans / duration:.3f}) | "
          f"diarization {t_diar:.0f}s (RTF {t_diar / duration:.3f}) | language={language or 'auto'}")
    talk = defaultdict(float)
    for s in diar["segments"]:
        talk[s["label"]] += s["end"] - s["start"]
    profiles = []
    for sp in diar["speaker_profiles"]["speakers"]:
        label = str(sp["speaker"])
        profiles.append({"slot": sp["speaker"], "person": None, "clean_duration": sp["clean_duration"],
                         "talk_seconds": round(talk.get(label, 0.0), 1), "embedding": sp.get("embedding")})
    print(f"speakers: {len(profiles)} -> " + ", ".join(
        f"slot {p['slot']}: {p['talk_seconds'] / 60:.1f} min talk, {p['clean_duration']} s clean" for p in profiles))
    json.dump({"meeting": name, "encoder": diar["speaker_profiles"]["encoder"], "profiles": profiles},
              open(out / f"{name}-profiles.json", "w"))
    if captions:
        ref = norm(vtt_text(captions))
        hyp = norm(" ".join(s["text"] for s in trans["segments"]))
        print(f"disagreement with YouTube auto-captions: {wer(ref, hyp):.1%} (captions {len(ref)} words, ours {len(hyp)} words)")


if __name__ == "__main__":
    main()
