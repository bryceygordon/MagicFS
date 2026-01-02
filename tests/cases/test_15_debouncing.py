from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 15: Search Debouncing (Typewriter Suppression) ---")

# 1. Setup: Clean the log file
initial_log_size = 0
if os.path.exists(test.log_file):
    initial_log_size = os.path.getsize(test.log_file)

# 2. Simulate Rapid Typing (Sequential but Fast)
# We want to emulate a user typing "magicfs".
# Order matters: 'm' (Oldest) -> 'magicfs' (Newest).
# The Oracle should prioritize the Newest.

prefixes = ["m", "ma", "mag", "magi", "magic", "magicf", "magicfs"]
final_query = "magicfs"

print(f"[Action] Rapidly triggering {len(prefixes)} search prefixes...")

# We loop fast. The Oracle has a 20ms accumulation delay.
# Python can easily fire 7 stat() calls in <5ms.
for query_str in prefixes:
    path = os.path.join(test.mount_point, "search", query_str)
    try:
        # Just trigger the syscall, don't wait for result
        os.stat(path)
    except OSError:
        pass
    # Tiny yield to ensure kernel processes syscalls in order, 
    # establishing the correct LRU order in InodeStore.
    # 0.001s = 1ms. 7ms total. Well within 20ms window.
    time.sleep(0.001)

# 3. Wait for Backpressure release
# The Oracle sleeps for 50ms loops + 20ms accumulation.
print("[Wait] Waiting for Oracle to process final query...")
time.sleep(2.0) 

# 4. Analyze Logs
print("[Analysis] Inspecting logs for suppression...")
dispatched_queries = []

try:
    with open(test.log_file, "r") as f:
        f.seek(initial_log_size)
        content = f.read()
        
        for line in content.splitlines():
            if "[Oracle] Dispatching search for:" in line:
                parts = line.strip().split("'")
                if len(parts) >= 2:
                    q = parts[1]
                    if q in prefixes:
                        dispatched_queries.append(q)
            elif "Batch Inbox" in line:
                print(f"  [Debug] {line.strip()}")

except FileNotFoundError:
    print("❌ FAILURE: Log file not found!")
    sys.exit(1)

print(f"  Dispatched: {dispatched_queries}")

# 5. Assertions
if len(dispatched_queries) == len(prefixes):
    print(f"❌ FAILURE: Debouncing failed! Processed all {len(prefixes)} prefixes.")
    sys.exit(1)

if final_query not in dispatched_queries:
    print(f"❌ FAILURE: The final query '{final_query}' was not dispatched!")
    print(f"  Found: {dispatched_queries}")
    sys.exit(1)

# Ideally we want mostly the final one.
if len(dispatched_queries) > 3:
    print(f"⚠️  Warning: {len(dispatched_queries)} queries processed. Debouncing is weak.")
    sys.exit(1) 

print(f"✅ SUCCESS: Suppressed {len(prefixes) - len(dispatched_queries)} intermediate queries.")
print("✅ DEBOUNCING TEST PASSED")
