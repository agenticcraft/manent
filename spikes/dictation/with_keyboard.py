# Run a command in the isolated session with a (virtual) keyboard on the seat: wl-copy refuses
# to work on a seat without one ("This seat has no keyboard"), which a real desktop always has.
#   spikes/dictation/run-isolated.sh python3 spikes/dictation/with_keyboard.py <command>...
import os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hands import Hands, mutter_eis
from ei import Keyboard
hands = Hands()  # keep the Mutter session alive while the command runs
kb = Keyboard(mutter_eis(hands))
# Keep serving the keyboard's events while the command runs (an unserved EIS client stalls
# the compositor's clipboard focus handoff).
p = subprocess.Popen(sys.argv[1:])
while p.poll() is None:
    kb.pump(0.05)
sys.exit(p.returncode)
