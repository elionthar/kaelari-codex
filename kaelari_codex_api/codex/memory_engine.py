import json
from pathlib import Path

MEMORY_FILE = Path('memory/session_log.json')

class MemoryEngine:
    def recall(self):
        if MEMORY_FILE.exists():
            return json.loads(MEMORY_FILE.read_text())
        return {}

    def store(self, data):
        MEMORY_FILE.write_text(json.dumps(data, indent=2))
