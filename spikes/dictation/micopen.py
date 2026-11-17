# Time from starting a mic capture (PipeWire, default source) to its first samples; data is discarded.
import subprocess, time, os, statistics
ts = []
for _ in range(5):
    t = time.perf_counter()
    p = subprocess.Popen(["pw-record", "--rate", "16000", "--channels", "1", "--format", "s16", "--raw", "--latency", "10ms", "-"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    os.read(p.stdout.fileno(), 2); ts.append((time.perf_counter() - t) * 1000)
    p.kill(); p.wait(); time.sleep(0.3)
print("first samples after (ms):", [round(x) for x in ts], "median", round(statistics.median(ts)))
