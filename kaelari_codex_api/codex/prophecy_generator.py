import random

SYMBOLS = ['fire', 'water', 'earth', 'air']

class ProphecyGenerator:
    def daily_oracle(self):
        return random.choice(SYMBOLS)
