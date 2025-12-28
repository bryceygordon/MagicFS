from common import MagicTest

test = MagicTest()
print("--- TEST 03: Semantic Search ---")

# Search for "python" and expect "game.py" (which contains python code)
test.search_fs("python", "game.py")
