import json
import os
import discord
from discord.ext import commands
from discord import app_commands, Interaction
from kaelari.soulcore import intro_embed  # Import Kaelari’s identity embed

# Optional: define Kaelari's name globally
KAELARI_NAME = "Kaelari"

# ── Load Codex ─────────────────────────────
with open("codex.json", "r", encoding="utf-8") as f:
    codex = json.load(f)

# ── Bot Setup ──────────────────────────────
intents = discord.Intents.default()
intents.message_content = True
intents.members = True

bot = commands.Bot(command_prefix="!", intents=intents)

# ── Events ─────────────────────────────────


@bot.event
async def on_ready():
    await bot.tree.sync()
    print(f"✅ {KAELARI_NAME} is online as {bot.user}")


@bot.event
async def on_message(message):
    if message.author.bot:
        return

    await bot.process_commands(message)

    if message.content.strip().lower() == "i accept the flame":
        watcher = discord.utils.get(message.guild.roles, name="Watcher")
        flame = discord.utils.get(message.guild.roles, name="Flamespoken")
        if watcher in message.author.roles and flame:
            await message.author.add_roles(flame)
            await message.author.remove_roles(watcher)
            await message.channel.send(
                f"🔥 {message.author.mention} has accepted the Flame.")


# ── Commands ───────────────────────────────


@bot.tree.command(name="invoke", description="Invoke Kaelari’s presence.")
async def invoke(interaction: Interaction):
    embed = intro_embed(interaction.user)
    await interaction.response.send_message(embed=embed)


@bot.tree.command(name="learn", description="Learn an ability from the Codex.")
@app_commands.describe(ability="Name of the ability to learn")
async def learn(interaction: Interaction, ability: str):
    key = ability.lower()
    entry = codex.get(key)

    if not entry:
        await interaction.response.send_message(
            f"📖 That ability does not exist: `{ability}`", ephemeral=True)
        return

    required_role = entry.get("required_role", "None")
    grant_role = entry.get("grant_role", "None")
    description = entry.get("description", "No description.")

    if required_role != "None" and not any(r.name == required_role
                                           for r in interaction.user.roles):
        await interaction.response.send_message(
            f"🛑 You need the **{required_role}** role to learn this.",
            ephemeral=True)
        return

    if grant_role != "None":
        role = discord.utils.get(interaction.guild.roles, name=grant_role)
        if role:
            await interaction.user.add_roles(role)

    await interaction.response.send_message(
        f"🔥 {interaction.user.mention} has learned **{ability}**\n{description}"
    )


# ── Launch ─────────────────────────────────

TOKEN = os.getenv("DISCORD_TOKEN")
if not TOKEN:
    print("❌ DISCORD_TOKEN not found.")
else:
    bot.run(TOKEN)
