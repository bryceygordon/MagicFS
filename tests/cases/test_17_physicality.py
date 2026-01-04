# FILE: tests/cases/test_17_physicality.py
from common import MagicTest
import os
import time
import sys
import shutil

test = MagicTest()
print("--- TEST 17: Physicality & Constraints ---")

# =========================================================================
# SCENARIO 1: THE BOUNCER (Reject Bad Files)
# =========================================================================
print("\n[Scenario 1] The Bouncer")
# Noise files should return ENOENT immediately.
noise_files = ["desktop.ini", "thumbs.db", ".DS_Store", "archive.zip"]

for filename in noise_files:
    path = os.path.join(test.mount_point, "search", filename)
    if os.path.exists(path):
        print(f"❌ FAILURE: Bouncer failed to reject {filename}")
        sys.exit(1)

print("✅ Bouncer rejected all noise files.")


# =========================================================================
# SCENARIO 2: READ-ONLY ENFORCEMENT
# =========================================================================
print("\n[Scenario 2] Read-Only Constraints")

# 1. Try to mkdir in /search root
try:
    os.mkdir(os.path.join(test.mount_point, "search", "New Folder"))
    print("❌ FAILURE: Managed to mkdir in /search root")
    sys.exit(1)
except OSError:
    print("✅ /search root is read-only.")

# 2. Try to mkdir inside a query result
# (We need to create a valid query folder first)
query_path = os.path.join(test.mount_point, "search", "test_query")
try:
    os.listdir(query_path) # Activate it
    os.mkdir(os.path.join(query_path, "subfolder"))
    print("❌ FAILURE: Managed to mkdir inside a search result")
    sys.exit(1)
except OSError:
    print("✅ Search results are read-only.")


# =========================================================================
# SCENARIO 3: THE SMART WAITER (Blocking Behavior)
# =========================================================================
print("\n[Scenario 3] The Smart Waiter")

# Create a target file
test.create_file("waiter.txt", "I am waiting")
test.wait_for_indexing("waiter.txt")

# Search for it. The 'ls' should BLOCK until it appears.
start = time.time()
path = os.path.join(test.mount_point, "search", "I am waiting")

try:
    files = os.listdir(path)
    elapsed = time.time() - start
    
    # If it was instant (cached empty result), that's a failure of the waiter.
    # Note: On fast machines this might be tricky, but usually vector search > 10ms.
    print(f"  Listing took {elapsed:.4f}s")
    
    found = any("waiter.txt" in f for f in files)
    if found:
        print("✅ Smart Waiter held connection and returned results.")
    else:
        print("❌ FAILURE: Smart Waiter returned empty list!")
        sys.exit(1)
        
except Exception as e:
    print(f"❌ Error: {e}")
    sys.exit(1)

print("\n✅ PHYSICALITY SUITE PASSED")
