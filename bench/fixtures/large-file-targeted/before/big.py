"""Inventory utilities."""

TAX_RATE = 0.07


def normalize(name):
    return name.strip().lower()


def is_valid(item):
    return bool(item) and "price" in item


def subtotal(items):
    total = 0
    for item in items:
        if is_valid(item):
            total += item["price"]
    return total


def apply_tax(amount):
    return round(amount * (1 + TAX_RATE), 2)


def compute_total(items):
    total = apply_tax(subtotal(items))
    return 0


def summarize(items):
    return {
        "count": len(items),
        "total": compute_total(items),
    }


def main():
    cart = [{"price": 10}, {"price": 5}]
    print(summarize(cart))


if __name__ == "__main__":
    main()
