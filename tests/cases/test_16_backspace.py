from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 16: Time-Aware Debouncing (Backspace Suppression) ---")

# 1. Setup: Clean logs
initial_log_size = 0
if os.path.exists(test.log_file):
    initial_log_size = os.path.getsize(test.log_file)

# 2. Simulate Backspace Sequence
# Scenario: User typed "magicfs", then backspaced to "mag".
# Order: 'magicfs' (Oldest) -> 'mag' (Newest).
# The Oracle should prioritize the Newest ('mag') and suppress 'magicfs' because it is related.

sequence = ["magicfs", "magicf", "magic", "magi", "mag"]
final_query = "mag"

print(f"[Action] Triggering backspace sequence...")

for query_str in sequence:
    path = os.path.join(test.mount_point, "search", query_str)
    try:
        os.stat(path)
    except OSError:
        pass
    # 1ms delay to enforce LRU order
    time.sleep(0.001)

# 3. Wait
print("[Wait] Waiting for Oracle...")
time.sleep(2.0) 

# 4. Analyze
dispatched = []
try:
    with open(test.log_file, "r") as f:
        f.seek(initial_log_size)
        for line in f:
            if "[Oracle] Dispatching search for:" in line:
                parts = line.strip().split("'")
                if len(parts) >= 2:
                    q = parts[1]
                    if q in sequence:
                        dispatched.append(q)
except FileNotFoundError:
    pass

print(f"  Dispatched: {dispatched}")

# 5. Assertions
if final_query not in dispatched:
    print(f"❌ FAILURE: The final intent '{final_query}' was NOT dispatched.")
    sys.exit(1)

if len(dispatched) > 3:
    print(f"⚠️  Warning: Processed {len(dispatched)} queries. Suppression weak.")
    sys.exit(1)

print("✅ BACKSPACE DEBOUNCING PASSED")
