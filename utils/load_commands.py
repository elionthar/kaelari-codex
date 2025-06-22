from importlib import import_module
from pathlib import Path


def load_commands(bot):
    commands_path = Path(__file__).resolve().parent.parent / "commands"
    for file in commands_path.glob("*.py"):
        if file.name.startswith("__"):
            continue
        module = import_module(f"commands.{file.stem}")
        if hasattr(module, "setup"):
            module.setup(bot)
