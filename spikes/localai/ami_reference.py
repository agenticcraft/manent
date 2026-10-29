"""VRT-31: turn AMI manual annotations into a reference for one meeting.

Usage: ami_reference.py <annotations dir> <meeting id> <out.json>

Output: {"meeting", "speakers": {agent: global_name}, "words": [{start, end, word, speaker}],
         "turns": [{start, end, speaker}]}, speakers as AMI global participant names
(stable across meetings, e.g. FEE013), words sorted by start time, punctuation dropped.
"""
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

NITE = "{http://nite.sourceforge.net/}"


def speakers_of(ann, meeting):
    root = ET.parse(ann / "corpusResources" / "meetings.xml").getroot()
    for m in root.iter("meeting"):
        if m.get("observation") == meeting:
            return {s.get("nxt_agent"): s.get("global_name") for s in m.iter("speaker")}
    raise SystemExit(f"meeting {meeting} not found")


def main(ann, meeting, out):
    ann = Path(ann)
    speakers = speakers_of(ann, meeting)
    words, turns = [], []
    for agent, name in speakers.items():
        wfile = ann / "words" / f"{meeting}.{agent}.words.xml"
        for w in ET.parse(wfile).getroot():
            if w.tag != "w" or w.get("punc") == "true" or not (w.text or "").strip():
                continue
            words.append({"start": float(w.get("starttime")), "end": float(w.get("endtime")),
                          "word": w.text.strip(), "speaker": name})
        sfile = ann / "segments" / f"{meeting}.{agent}.segments.xml"
        for s in ET.parse(sfile).getroot():
            if s.tag == "segment":
                turns.append({"start": float(s.get("transcriber_start")), "end": float(s.get("transcriber_end")),
                              "speaker": name})
    words.sort(key=lambda w: w["start"])
    turns.sort(key=lambda t: t["start"])
    json.dump({"meeting": meeting, "speakers": speakers, "words": words, "turns": turns}, open(out, "w"), indent=1)
    speech = sum(t["end"] - t["start"] for t in turns)
    print(f"{meeting}: {len(speakers)} speakers {sorted(speakers.values())}, {len(words)} words, "
          f"{len(turns)} turns, {speech / 60:.1f} min of speech (incl. overlap)")


if __name__ == "__main__":
    main(*sys.argv[1:4])
