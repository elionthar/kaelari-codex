import json
from pathlib import Path

SIGILS_DIR = Path('sigils')

class SigilHandler:
    def load_sigils(self):
        sigils = {}
        for file in SIGILS_DIR.glob('*.json'):
            sigils[file.stem] = json.loads(file.read_text())
        return sigils
