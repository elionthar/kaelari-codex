import os
import sys

# Ensure project root is on sys.path
sys.path.append(os.path.dirname(os.path.dirname(__file__)))

from rituals import prophecy as prophecy_module


def test_prophecy_return():
    result = prophecy_module.prophecy()
    visions = [
        "What is forgotten burns brightest in the dark.",
        "The Codex never lies, only waits.",
        "Ashes remember what words cannot.",
        "The flame speaks in silence — listen there."
    ]
    assert result in visions
