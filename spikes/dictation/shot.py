import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hands import shot
print(shot(sys.argv[1]))
