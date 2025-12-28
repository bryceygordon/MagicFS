from common import MagicTest

test = MagicTest()
print("--- TEST 02: Ignore Rules (.magicfsignore) ---")

# 1. Standard Dotfiles (Might pass with hardcoded logic)
test.assert_file_not_indexed(".git/config")
test.assert_file_not_indexed(".obsidian/workspace.json")

# 2. Custom Ignore Rules (Will FAIL without .magicfsignore logic)
# "secrets" is a normal folder name, so only the ignore file can catch it.
test.assert_file_not_indexed("secrets/passwords.txt")
