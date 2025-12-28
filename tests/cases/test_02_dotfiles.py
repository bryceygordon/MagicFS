from common import MagicTest
import time

test = MagicTest()
print("--- TEST 02: Dynamic Ignore Rules ---")

# 1. Set up ignore rules
# We must explicitly add .git if we want to test that it gets ignored!
test.add_ignore_rule("secrets")
test.add_ignore_rule(".git")

# 2. Create ignored content
test.create_file("secrets/password.txt", "super_secret")
test.create_file(".git/config", "repo_config")

# 3. Create normal content to ensure system is still working
# This acts as a "barrier" to ensure the previous file events have been processed
test.create_file("public/readme.md", "# Public Info")
test.wait_for_indexing("readme.md")

# 4. Assertions
test.assert_file_not_indexed("password.txt")
test.assert_file_not_indexed("config") 
test.assert_file_indexed("readme.md")
