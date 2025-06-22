import os
from dotenv import load_dotenv

# Load environment variables from .env or secrets.env
load_dotenv(dotenv_path=os.getenv('ENV_PATH', '.env'))

DISCORD_TOKEN = os.getenv('DISCORD_TOKEN')
CODEX_SETTINGS = {
    'memory_path': os.getenv('MEMORY_PATH', 'memory/session_log.json')
}
