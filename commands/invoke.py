import discord
from discord import app_commands
from rituals.invoke import invoke

async def invoke_command(interaction: discord.Interaction):
    result = invoke()
    await interaction.response.send_message(result)

def setup(bot):
    bot.tree.add_command(app_commands.Command(
        name="invoke",
        description="Invoke a ritual",
        callback=invoke_command
    ))
