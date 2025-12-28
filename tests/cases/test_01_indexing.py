from common import MagicTest

test = MagicTest()
print("--- TEST 01: Dynamic Indexing ---")

# 1. Create file dynamically
test.create_file("game.py", "import snake\nprint('hiss')")

# 2. Wait for Librarian -> Oracle pipeline
test.wait_for_indexing("game.py")

# 3. Assert
test.assert_file_indexed("game.py")
