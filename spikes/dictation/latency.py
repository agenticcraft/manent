# Time the engine helper's transcribe op on short clips (warm, 3 runs each).
import json, subprocess, sys, time, os, statistics
exe, lib, model, *wavs = sys.argv[1:]
p = subprocess.Popen([exe, "--lib", lib, "--model", model], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, env={**os.environ, "LD_LIBRARY_PATH": os.path.dirname(lib)})
print("ready:", p.stdout.readline().strip()[:120])
n = 0
def req(wav):
    global n; n += 1
    t = time.perf_counter(); p.stdin.write(json.dumps({"id": n, "op": "transcribe", "wav": wav}) + "\n"); p.stdin.flush()
    r = json.loads(p.stdout.readline()); return time.perf_counter() - t, r
req(wavs[0])  # warm up
for w in wavs:
    times = []; text = ""
    for _ in range(3):
        dt, r = req(w); times.append(dt); text = r.get("result", {}).get("text", r.get("error"))
    print(f"{os.path.basename(w):10s} median {statistics.median(times)*1000:6.0f} ms  (min {min(times)*1000:.0f}, max {max(times)*1000:.0f})  {text[:70]!r}")
p.stdin.close(); p.wait()
