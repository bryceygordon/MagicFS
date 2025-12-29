from common import MagicTest
import os
import time

test = MagicTest()
print("--- TEST 03: Semantic Search ---")

# 1. Create content
# This creates a NEW directory 'projects'. 
# The updated create_file has a tiny sleep to help notify catch up.
test.create_file("projects/ai.rs", "fn main() { println!(\"vector search implementation\"); }")

# 2. Wait for indexing (Hard Fails now if missed)
test.wait_for_indexing("ai.rs")

# 3. Search
query = "vector search"
expected = "ai.rs"
print(f"[*] Searching for '{query}'...")

search_path = os.path.join(test.mount_point, "search", query)
found = False

for i in range(15):
    try:
        if os.path.exists(search_path):
            results = os.listdir(search_path)
            # Check if ANY file in results matches expected substring
            if any(expected in r for r in results):
                print(f"✅ Found '{expected}' in search results.")
                found = True
                break
            elif len(results) > 0:
                 # It exists and has files, but not ours?
                 pass
    except OSError:
        pass 
    
    print(f"   ... waiting for Oracle (attempt {i+1}/15)")
    time.sleep(0.5)

if not found:
    print(f"❌ FAILURE: Search for '{query}' failed.")
    if os.path.exists(search_path):
        print(f"   Directory contents: {os.listdir(search_path)}")
    else:
        print("   Directory does not exist (EAGAIN loop persisted).")
    exit(1)
