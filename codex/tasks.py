import importlib
from rituals.bind import bind
from rituals.prophecy import prophecy
from rituals.summon import summon

TASKS = {
    "bind": bind,
    "prophecy": prophecy,
    "summon": summon,
}


def run_task(name: str):
    """Execute a task by name and return its result."""
    task = TASKS.get(name)
    if not task:
        raise ValueError(f"Unknown task: {name}")
    return task()
