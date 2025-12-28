from common import MagicTest

test = MagicTest()
print("--- TEST 03: Semantic Search ---")

# 1. Create content
# This creates a NEW directory 'projects'. 
# The updated create_file has a tiny sleep to help notify catch up.
test.create_file("projects/ai.rs", "// This is a rust vector search implementation")

# 2. Wait for indexing (Hard Fails now if missed)
test.wait_for_indexing("ai.rs")

# 3. Search
test.search_fs("vector search", "ai.rs")
