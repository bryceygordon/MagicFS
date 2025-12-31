# FILE: tests/cases/test_09_chatter.py
from common import MagicTest
import time
import os

test = MagicTest()
print("--- TEST 09: Thermal Protection (Chatterbox) ---")

log_path = "tests/magicfs.log"

# 1. Measure initial log length so we ignore old logs
initial_log_lines = 0
if os.path.exists(log_path):
    with open(log_path, "r") as f:
        initial_log_lines = sum(1 for _ in f)

filename = "server.log"
file_path = os.path.join(test.watch_dir, filename)

print(f"[Action] Hammering {filename} with 50 updates...")

# 2. The Hammer: 50 updates in ~2 seconds
for i in range(50):
    with open(file_path, "a") as f:
        f.write(f"Log entry {i}: System is doing something...\n")
    time.sleep(0.05) 

print("[Wait] Waiting for system to settle...")
test.wait_for_stable_db(stability_duration=3)

# 3. Analyze Logs (Read only new lines)
print("[Analysis] Counting index operations...")
index_count = 0
current_line = 0
with open(log_path, "r") as f:
    for line in f:
        current_line += 1
        if current_line <= initial_log_lines:
            continue
            
        if f"[Indexer] Processing: {test.watch_dir}/{filename}" in line:
            index_count += 1

print(f"Updates Sent: 50")
print(f"Index Operations: {index_count}")

# 4. Assert Efficiency
THRESHOLD = 10

if index_count > THRESHOLD:
    print(f"❌ FAILURE: Thermal protection failed. System processed {index_count} updates (Threshold: {THRESHOLD}).")
    exit(1)
elif index_count == 0:
    # Double check DB before failing
    if test.check_file_in_db(filename):
        print("⚠️  Warning: Log grep found 0 ops, but file IS in DB. This is a logging lag/race.")
        # We allow it if DB is correct, but ideally we want to see the logs.
    else:
        print(f"❌ FAILURE: System ignored the file completely!")
        exit(1)

print(f"✅ SUCCESS: Chatter suppressed. Only {index_count} operations performed.")
    
# 5. Verify Data Integrity (The "Final Promise")
test.wait_for_indexing(filename)
last_entry_query = "Log entry 49"
print(f"[Verify] Searching for final entry: '{last_entry_query}'")

# Give it a moment to ensure the Final Promise flush happened
time.sleep(2.0)

if test.search_fs(last_entry_query, filename):
    print("✅ Final state consistency verified.")
else:
    print("❌ FAILURE: The final update was lost! (Over-debounced)")
    exit(1)
