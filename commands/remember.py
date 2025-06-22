import json
from pathlib import Path
from datetime import datetime
import discord
from discord import app_commands

LOG_FILE = Path(__file__).resolve().parent.parent / "memory" / "session_log.json"

@app_commands.command(name="remember", description="Store a note for this session")
@app_commands.describe(message="Message to remember")
async def remember(interaction: discord.Interaction, message: str):
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    if LOG_FILE.exists():
        try:
            data = json.load(open(LOG_FILE))
        except json.JSONDecodeError:
            data = []
    else:
        data = []
    if not isinstance(data, list):
        data = [data]
    entry = {
        "user": interaction.user.name,
        "message": message,
        "timestamp": datetime.utcnow().isoformat()
    }
    data.append(entry)
    with open(LOG_FILE, "w") as f:
        json.dump(data, f, indent=2)
    await interaction.response.send_message("Noted.", ephemeral=True)

def setup(bot):
    bot.tree.add_command(remember)
