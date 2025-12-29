from common import MagicTest

test = MagicTest()
print("--- TEST 03: Semantic Search ---")

# 1. Create content
test.create_file("projects/ai.rs", "fn main() { println!(\"vector search implementation\"); }")

# 2. Wait for indexing
test.wait_for_indexing("ai.rs")

# 3. Search
# We now use the built-in search_fs which has 15s timeout and log dumping
test.search_fs("vector search", "ai.rs")
