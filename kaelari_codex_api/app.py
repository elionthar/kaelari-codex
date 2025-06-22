from flask import Flask

app = Flask(__name__)

# Import routes
from .routes.invoke import invoke_blueprint
from .routes.memory import memory_blueprint
from .routes.ritual import ritual_blueprint
from .routes.prophecy import prophecy_blueprint

app.register_blueprint(invoke_blueprint)
app.register_blueprint(memory_blueprint)
app.register_blueprint(ritual_blueprint)
app.register_blueprint(prophecy_blueprint)

if __name__ == "__main__":
    app.run()
