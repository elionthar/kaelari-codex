import os
import json
from datetime import datetime

import discord
from discord.ext import commands
from github import Github

from codex.tasks import run_task

MEMORY_DIR = os.path.join(os.path.dirname(__file__), '..', 'memory')
LOG_FILE = os.path.join(MEMORY_DIR, 'discord_log.jsonl')


def log_event(event: dict) -> None:
    """Append a JSON event to the discord log."""
    os.makedirs(MEMORY_DIR, exist_ok=True)
    event['timestamp'] = datetime.utcnow().isoformat()
    with open(LOG_FILE, 'a') as f:
        f.write(json.dumps(event) + '\n')


intents = discord.Intents.default()
bot = commands.Bot(command_prefix='!', intents=intents)

github_token = os.environ.get('GITHUB_TOKEN')
gh = Github(github_token) if github_token else Github()


@bot.event
async def on_ready():
    print(f'Connected as {bot.user}')
    log_event({'event': 'ready', 'user': str(bot.user)})


@bot.command(name='task')
async def run_codex_task(ctx: commands.Context, *, name: str):
    """Run a Codex task by name."""
    try:
        result = run_task(name)
        log_event({'event': 'task', 'task': name, 'user': str(ctx.author)})
        await ctx.reply(f'Task `{name}` executed: {result}')
    except Exception as exc:
        await ctx.reply(f'Error: {exc}')


@bot.command(name='latest')
async def latest_commit(ctx: commands.Context, repo: str):
    """Return the latest commit hash for a GitHub repo (owner/repo)."""
    try:
        repository = gh.get_repo(repo)
        commit = repository.get_commits()[0]
        await ctx.reply(f'Latest commit in {repo}: {commit.sha[:7]}')
        log_event({'event': 'latest', 'repo': repo, 'user': str(ctx.author)})
    except Exception as exc:
        await ctx.reply(f'GitHub error: {exc}')


def main() -> None:
    token = os.environ.get('DISCORD_BOT_TOKEN')
    if not token:
        raise RuntimeError('DISCORD_BOT_TOKEN not set in environment')
    bot.run(token)


if __name__ == '__main__':
    main()
