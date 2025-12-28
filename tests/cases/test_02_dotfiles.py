from common import MagicTest

test = MagicTest()
print("--- TEST 02: Ignore Rules (.magicfsignore) ---")

# These files exist in the directory but should NOT be in the DB
test.assert_file_not_indexed(".git/config")
test.assert_file_not_indexed(".obsidian/workspace.json")
