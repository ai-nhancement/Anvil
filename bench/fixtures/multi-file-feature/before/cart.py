class Cart:
    def __init__(self):
        self.items = []

    def add(self, name, price):
        self.items.append((name, price))
