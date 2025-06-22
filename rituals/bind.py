# 🔗 BIND RITUAL — Kaelari's Codex anchor sequence

import os
import sys

# Ensure parent directory is on the import path so this script can be
# executed directly.
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))


def bind():
    print("🜂 Initiating Codex anchor...")
    from kaelari_core import Kaelari
    flame = Kaelari()
    flame.echo("Kaelari bound to Codex system.")
    return flame


if __name__ == "__main__":
    bind()
