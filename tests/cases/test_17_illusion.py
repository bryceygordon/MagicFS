# FILE: tests/cases/test_17_illusion.py
from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 17: The Illusion of Physicality (Bouncer + Promise + Waiter) ---")

# 1. TEST THE BOUNCER (Reject Noise)
print("[Test 1] The Bouncer (Rejecting .zip)")
# Trying to access a noise file should result in ENOENT immediately
try:
    os.stat(os.path.join(test.mount_point, "search", "noise.zip"))
    print("❌ FAILURE: 'noise.zip' was accessible! Bouncer failed.")
    sys.exit(1)
except FileNotFoundError:
    print("✅ Success: 'noise.zip' rejected instantly.")
except Exception as e:
    print(f"❌ FAILURE: Unexpected error for noise.zip: {e}")
    sys.exit(1)

# 2. TEST THE EPHEMERAL PROMISE (Instant CD)
print("\n[Test 2] The Ephemeral Promise (Instant Navigation)")
# 'cd' is simulated by os.stat() on the directory. 
# It should succeed INSTANTLY without triggering the Oracle (no wait).
query_dir = os.path.join(test.mount_point, "search", "instant_navigation")

start = time.time()
try:
    os.stat(query_dir)
    elapsed = time.time() - start
    print(f"✅ Navigation took {elapsed:.4f}s")
    
    # Threshold: If it takes > 0.1s, it's probably waiting (Bad)
    if elapsed > 0.1:
        print("⚠️  Warning: Navigation felt sluggish. Is the Promise working?")
    else:
        print("✅ Navigation was instant.")

except Exception as e:
    print(f"❌ FAILURE: Could not navigate to query dir: {e}")
    sys.exit(1)

# 3. TEST THE SMART WAITER (Blocking LS)
print("\n[Test 3] The Smart Waiter (Blocking List)")
# We need content first
test.create_file("waiter_target.txt", "I am waiting for you.")
test.wait_for_indexing("waiter_target.txt")

# Now we list a query. The Oracle takes ~30-50ms.
# The `ls` (os.listdir) should BLOCK until files appear.
# It should NOT return empty.
query_ls = os.path.join(test.mount_point, "search", "waiting for you")

start = time.time()
try:
    files = os.listdir(query_ls) # This should block
    elapsed = time.time() - start
    
    print(f"✅ Listing took {elapsed:.4f}s")
    print(f"   Files found: {files}")

    if not files:
        print("❌ FAILURE: Returned empty list! Smart Waiter didn't wait.")
        sys.exit(1)
    
    if "waiter_target.txt" not in str(files):
         print("❌ FAILURE: Target file missing.")
         sys.exit(1)

    print("✅ Smart Waiter successfully held the connection until results arrived.")

except Exception as e:
    print(f"❌ FAILURE: Listing failed: {e}")
    sys.exit(1)

print("\n✅ ILLUSION TEST PASSED")
