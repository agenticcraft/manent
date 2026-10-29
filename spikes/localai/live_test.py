"""VRT-2: stream a WAV to LocalAI's realtime WebSocket at real-time speed and
measure live transcription latency.

Usage: live_test.py <wav> <truth.json> [--model veritas-live-en] [--language en|nl] [--turn semantic_vad|server_vad]
                    [--url ws://127.0.0.1:8181]

The WAV must be 16 kHz mono PCM16 or float32. Audio is sent in 100 ms chunks,
paced to the wall clock. Every server event is logged with the audio position
(seconds of audio sent so far). Afterwards, per reference turn:
  - first-caption latency: first delta after the turn started, minus turn start
  - final latency: the 'completed' transcript covering the turn, minus turn end
"""
import asyncio
import base64
import json
import sys
import time

import numpy as np
import websockets

from wavio import read_wav

CHUNK_S = 0.1


async def run(wav, truth_path, model, url, language=None, turn="semantic_vad"):
    sr, x = read_wav(open(wav, "rb").read())
    assert sr == 16000, f"expected 16 kHz, got {sr}"
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2").tobytes()
    truth = json.load(open(truth_path))["segments"]
    events, t0 = [], None
    sent_s = 0.0

    async with websockets.connect(f"{url}/v1/realtime?model={model}", max_size=None) as ws:
        await ws.send(json.dumps({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {"input": {
                    "format": {"type": "audio/pcm", "rate": sr},
                    "transcription": {"model": model, **({"language": language} if language else {})},
                    "turn_detection": {"type": turn, **({"eagerness": "medium"} if turn == "semantic_vad" else {})},
                }},
            },
        }))

        async def receiver():
            async for msg in ws:
                ev = json.loads(msg)
                now = time.monotonic() - t0 if t0 else 0.0
                events.append((now, sent_s, ev))
                kind = ev.get("type", "")
                detail = ev.get("delta") or ev.get("transcript") or ev.get("speaker_name") or ev.get("speaker") or ev.get("error", "")
                if kind.endswith(".delta") and not ev.get("delta", "").strip():
                    continue
                print(f"  t={now:6.2f}s audio={sent_s:6.2f}s  {kind}  {str(detail)[:90]}")

        rx = asyncio.create_task(receiver())
        await asyncio.sleep(1.0)  # let session.updated arrive and models warm up
        t0 = time.monotonic()
        step = int(CHUNK_S * sr) * 2
        for i in range(0, len(pcm), step):
            chunk = pcm[i:i + step]
            await ws.send(json.dumps({"type": "input_audio_buffer.append", "audio": base64.b64encode(chunk).decode()}))
            sent_s += len(chunk) / 2 / sr
            # pace to real time
            delay = t0 + sent_s - time.monotonic()
            if delay > 0:
                await asyncio.sleep(delay)
        # trailing silence so the last turn can close
        silence = base64.b64encode(b"\0" * step).decode()
        for _ in range(int(6 / CHUNK_S)):
            await ws.send(json.dumps({"type": "input_audio_buffer.append", "audio": silence}))
            sent_s += CHUNK_S
            await asyncio.sleep(CHUNK_S)
        await asyncio.sleep(2)
        rx.cancel()

    json.dump([{"t": t, "audio": a, **ev} for t, a, ev in events], open(f"results/{wav.rsplit('/', 1)[-1][:-4]}-{model}-live-events.json", "w"))
    report(events, truth)


def report(events, truth):
    deltas = [(t, ev) for t, _, ev in events if ev.get("type", "").endswith("input_audio_transcription.delta") and ev.get("delta", "").strip()]
    completed = [(t, ev) for t, _, ev in events if ev.get("type", "").endswith("input_audio_transcription.completed")]
    segments = [ev for _, _, ev in events if ev.get("type", "").endswith("input_audio_transcription.segment")]
    speakers = [ev for _, _, ev in events if ev.get("type") == "conversation.item.speaker"]
    errors = [ev for _, _, ev in events if ev.get("type") == "error"]
    print(f"\nevents: {len(events)}  deltas: {len(deltas)}  completed: {len(completed)}  segments: {len(segments)}  speaker events: {len(speakers)}  errors: {len(errors)}")
    for e in errors[:5]:
        print("  error:", json.dumps(e)[:200])
    print("\nPer turn:")
    for turn in truth:
        first = next((t for t, _ in deltas if t >= turn["start"]), None)
        done = next((t for t, _ in completed if t >= turn["end"]), None)
        fc = f"{first - turn['start']:.2f}s" if first is not None else "-"
        fl = f"{done - turn['end']:.2f}s" if done is not None else "-"
        print(f"  [{turn['start']:5.1f}-{turn['end']:5.1f}] {turn['voice']}: first caption +{fc}, final +{fl}")
    print("\nCompleted transcripts:")
    for t, ev in completed:
        print(f"  t={t:6.2f}s  {ev.get('transcript', '')[:110]}")
    if segments:
        print("\nSpeaker segments:")
        for ev in segments:
            print(f"  speaker={ev.get('speaker')} name={ev.get('speaker_name')} {ev.get('start')}-{ev.get('end')}")


if __name__ == "__main__":
    args = sys.argv[1:]
    model = args[args.index("--model") + 1] if "--model" in args else "veritas-live-en"
    url = args[args.index("--url") + 1] if "--url" in args else "ws://127.0.0.1:8181"
    language = args[args.index("--language") + 1] if "--language" in args else None
    turn = args[args.index("--turn") + 1] if "--turn" in args else "semantic_vad"
    asyncio.run(run(args[0], args[1], model, url, language, turn))
