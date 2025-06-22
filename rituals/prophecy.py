# 🔮 PROPHECY RITUAL — Echoes seeded in Kaelari's flame

import random

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