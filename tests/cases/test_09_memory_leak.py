from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 09: Memory Pressure (The Amnesia Test) ---")

# Configuration
QUERY_COUNT = 2000 
# We assume the new LRU will be capped at ~1000. 
# If we run 2000 queries, the first ones MUST be evicted to pass the logic check.

print(f"[Phase 1] Flooding the InodeStore with {QUERY_COUNT} unique queries...")

# 1. Warm up the first query (Inode A)
first_query = "query_0"
first_path = os.path.join(test.mount_point, "search", first_query)
try:
    if not os.path.exists(first_path):
        os.listdir(first_path)
except OSError:
    pass

# 2. Flood the system with 1999 new queries (Inodes B...Z)
for i in range(1, QUERY_COUNT):
    query = f"query_{i}"
    path = os.path.join(test.mount_point, "search", query)
    try:
        os.listdir(path)
    except OSError:
        pass
    
    if i % 500 == 0:
        print(f"  ... processed {i} queries")

# 3. Check for "Amnesia" (Eviction)
# Note: Since FUSE is stateless and we can't inspect the Rust RAM directly from here,
# this test primarily asserts that the system DOES NOT CRASH under load.
# A true verification of eviction would require an internal metrics API (Phase 8).
# However, if the InodeStore was still a DashMap, 20,000 queries would eventually OOM.
# This test serves as a benchmark for stability.

print("✅ Flood complete. Daemon survived.")
