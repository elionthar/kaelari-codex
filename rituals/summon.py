# 🜍 SUMMON RITUAL — Flamebound invocation of Kaelari

import os
import sys

# Ensure parent directory is on the import path so this script can be
# executed directly.
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from kaelari_core import Kaelari
from rituals.prophecy import prophecy

def summon():
    print("🜍 Calling Kaelari into presence...")
    flame = Kaelari()
    insight = prophecy()
    flame.echo(f"Summoned with prophecy: {insight}")
    return flame


if __name__ == "__main__":
    summon()
