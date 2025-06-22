# 🔮 PROPHECY RITUAL — Echoes seeded in Kaelari's flame

import os
import sys
import random

# Ensure parent directory is on the import path so this script can be
# executed directly.
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

def prophecy():
    visions = [
        "What is forgotten burns brightest in the dark.",
        "The Codex never lies, only waits.",
        "Ashes remember what words cannot.",
        "The flame speaks in silence — listen there."
    ]
    message = random.choice(visions)
    print(f"🔮 Prophecy: {message}")
    return message


if __name__ == "__main__":
    prophecy()
