import importlib
import pkgutil

class ConstructLoader:
    def load_constructs(self, package):
        constructs = {}
        for _, name, _ in pkgutil.iter_modules(package.__path__):
            module = importlib.import_module(f"{package.__name__}.{name}")
            constructs[name] = module
        return constructs
