from common import MagicTest

test = MagicTest()
print("--- TEST 01: Basic File Indexing ---")

# Check if standard files are picked up
test.assert_file_indexed("game.py")
test.assert_file_indexed("main.rs")
