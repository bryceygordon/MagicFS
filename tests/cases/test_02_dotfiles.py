from common import MagicTest
import time
import os

test = MagicTest()
print("--- TEST 02: Dynamic Ignore Rules ---")

# 1. Create a file that SHOULD be indexed initially
test.create_file("secrets/password.txt", "super_secret")
test.wait_for_indexing("password.txt")

# 2. Add ignore rule
test.add_ignore_rule("secrets")
time.sleep(1) # Extra time for Librarian to reload

# 3. Create canary to prove Librarian processed the batch
test.create_file("public/readme.md", "# Public Info")
test.wait_for_indexing("readme.md")

# 4. Create file in ignored dir
test.create_file("secrets/new_password.txt", "even_more_secret")
time.sleep(1.5) # Wait for debounce + processing

# 5. Assert
test.assert_file_not_indexed("new_password.txt")
print("✅ Dynamic ignore rule is working.")
