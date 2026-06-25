from formatting import format_name

assert format_name("john smith") == "John Smith", f'format_name("john smith") = {format_name("john smith")!r}, want "John Smith"'
assert format_name("ada lovelace") == "Ada Lovelace", f'format_name("ada lovelace") = {format_name("ada lovelace")!r}, want "Ada Lovelace"'
print("all tests passed")
