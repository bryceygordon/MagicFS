from common import MagicTest
import os
import time
import sys
import concurrent.futures

test = MagicTest()
print("--- TEST 15: Search Debouncing (Typewriter Suppression) ---")

# 1. Setup: Clean the log file
# We use 'truncate' logic but we must be careful not to break the Daemon's pipe.
# Instead of deleting it, we note the current size/position.
initial_log_size = 0
if os.path.exists(test.log_file):
    initial_log_size = os.path.getsize(test.log_file)

# 2. Simulate Rapid Typing (Threaded)
prefixes = ["m", "ma", "mag", "magi", "magic", "magicf", "magicfs"]
final_query = "magicfs"

print(f"[Action] Rapidly triggering {len(prefixes)} search prefixes (Parallel)...")

def trigger_lookup(query_str):
    path = os.path.join(test.mount_point, "search", query_str)
    try:
        # Just trigger the syscall, don't wait for result
        os.stat(path)
    except OSError:
        pass

# Use ThreadPool to fire requests simultaneously, filling the FUSE queue
with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
    executor.map(trigger_lookup, prefixes)

# 3. Wait for Backpressure release
# The Oracle sleeps for 50ms loops + 50ms accumulation.
print("[Wait] Waiting for Oracle to process final query...")
time.sleep(3.0) 

# 4. Analyze Logs
print("[Analysis] Inspecting logs for suppression...")
dispatched_queries = []

# Read only NEW content
try:
    with open(test.log_file, "r") as f:
        f.seek(initial_log_size)
        content = f.read()
        
        for line in content.splitlines():
            if "[Oracle] 🚀 Dispatching search for:" in line:
                # Log format: ... [Oracle] 🚀 Dispatching search for: 'query'
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
    print("  DUMPING LOGS FOR CONTEXT:")
    print(content)
    sys.exit(1)

if final_query not in dispatched_queries:
    print(f"❌ FAILURE: The final query '{final_query}' was not dispatched!")
    print(f"  Found: {dispatched_queries}")
    print("  DUMPING LOGS FOR CONTEXT:")
    print(content)
    sys.exit(1)

# Ideally we want mostly the final one, but "m" might slip through if it was first.
if len(dispatched_queries) > 3:
    print(f"⚠️  Warning: {len(dispatched_queries)} queries processed. Debouncing is weak.")
    sys.exit(1) # Strict fail

print(f"✅ SUCCESS: Suppressed {len(prefixes) - len(dispatched_queries)} intermediate queries.")
print("✅ DEBOUNCING TEST PASSED")
