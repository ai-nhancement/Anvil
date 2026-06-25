from cart import Cart
from pricing import apply_tax

# apply_tax must apply the 8% TAX_RATE.
assert round(apply_tax(100.0), 2) == 108.0, f"apply_tax(100.0) = {apply_tax(100.0)}, want 108.0"

# Cart.total() = subtotal of item prices, with tax applied.
c = Cart()
c.add("book", 10.0)
c.add("pen", 2.0)
# subtotal 12.00, +8% tax = 12.96
assert round(c.total(), 2) == 12.96, f"total() = {c.total()}, want 12.96"

print("all tests passed")
