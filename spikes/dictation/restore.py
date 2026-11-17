import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rd import Portal
p = Portal(restore_token=sys.argv[1])
print("restore:", p.start[0] if p.start else None, "devices", p.start[1].get("devices") if p.start else None,
      "clipboard", p.start[1].get("clipboard_enabled") if p.start else None, f"start {p.start_ms:.0f} ms",
      "new token", bool(p.start and p.start[1].get("restore_token")), flush=True)
