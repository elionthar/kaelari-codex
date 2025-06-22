import random
from pathlib import Path
import discord
from discord import app_commands

PROPHECY_FILE = Path(__file__).resolve().parent.parent / "prophecies.txt"

async def prophecy(interaction: discord.Interaction):
    with open(PROPHECY_FILE, "r") as f:
        lines = [line.strip() for line in f if line.strip()]
    message = random.choice(lines)
    await interaction.response.send_message(message)

def setup(bot):
    bot.tree.add_command(app_commands.Command(
        name="prophecy",
        description="Reveal a flame prophecy",
        callback=prophecy
    ))
