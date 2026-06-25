from greet import greet

assert greet("Sam") == "Hello, Sam!", f'greet("Sam") = {greet("Sam")!r}, want "Hello, Sam!"'
assert greet("Sam", excited=True) == "Hello, Sam!!!", f'greet("Sam", excited=True) = {greet("Sam", excited=True)!r}, want "Hello, Sam!!!"'
print("all tests passed")
