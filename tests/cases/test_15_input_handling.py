# FILE: tests/cases/test_15_input_handling.py
from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 15: Input Handling & Autosuggest Suppression ---")

# Setup: Clean logs to track Oracle activity
initial_log_size = 0
if os.path.exists(test.log_file):
    initial_log_size = os.path.getsize(test.log_file)

def count_dispatches(since_log_pos):
    """Scans logs for '[Oracle] Dispatching search for:'"""
    count = 0
    try:
        with open(test.log_file, "r") as f:
            f.seek(since_log_pos)
            for line in f:
                if "[Oracle] Dispatching search for:" in line:
                    count += 1
    except FileNotFoundError:
        pass
    return count

# =========================================================================
# SCENARIO 1: THE SHELL AUTOSUGGEST (The "Lazy" Test)
# =========================================================================
print("\n[Scenario 1] Shell Autosuggest (Lookup without Entry)")
# We simulate a shell checking for existence: "w", "wa", "wal"...
# EXPECTATION: 0 Dispatches. 
# The inodes should be created, but the Oracle should sleep.

prefixes = ["w", "wa", "wal", "wall", "walle", "wallet"]
print(f"  Simulating shell typing: {prefixes}")

for p in prefixes:
    path = os.path.join(test.mount_point, "search", p)
    try:
        os.stat(path) # 'stat' triggers lookup(), NOT readdir()
    except OSError:
        pass
    time.sleep(0.01) # Human typing speed

# Wait 2 seconds (Oracle tick is 50ms, so this is plenty)
time.sleep(2.0)

dispatches = count_dispatches(initial_log_size)
print(f"  Oracle Dispatches: {dispatches}")

if dispatches > 0:
    print("❌ FAILURE: Autosuggest triggered the Oracle!")
    print("  'stat()' (lookup) should NOT trigger a search. Only 'ls' (readdir) should.")
    sys.exit(1)
else:
    print("✅ Success: Shell autosuggest was completely ignored.")


# =========================================================================
# SCENARIO 2: THE INTENTIONAL SEARCH (Lazy Loading)
# =========================================================================
print("\n[Scenario 2] The Intentional Search (Enter Directory)")
# Now we actually ENTER the final directory. This should trigger ONE dispatch.

target_query = "wallet"
target_path = os.path.join(test.mount_point, "search", target_query)

print(f"  User types 'cd {target_query}' & 'ls'...")
try:
    # readdir() triggers mark_active()
    os.listdir(target_path) 
except OSError:
    pass

# Wait for processing
time.sleep(1.0)

new_dispatches = count_dispatches(initial_log_size)
print(f"  Total Oracle Dispatches: {new_dispatches}")

if new_dispatches == 1:
    print("✅ Success: Oracle triggered exactly once upon entry.")
elif new_dispatches == 0:
    print("❌ FAILURE: Oracle IGNORED the directory entry! Search broken.")
    sys.exit(1)
else:
    print(f"⚠️  Warning: Multiple dispatches ({new_dispatches}). Debouncing might be loose.")


# =========================================================================
# SCENARIO 3: QUOTE SANITIZATION
# =========================================================================
print("\n[Scenario 3] Quote Sanitization")
# User types: cd "my query"
# MagicFS receives: "my query" (quotes included)
# It should strip them.

raw_query = '"quoted query"'
sanitized_intent = "quoted query"

path = os.path.join(test.mount_point, "search", raw_query)
try:
    os.stat(path) # Create inode
    os.listdir(path) # Trigger search
except OSError:
    pass

time.sleep(1.0)

# Scan logs to see what was actually dispatched
found_sanitized = False
with open(test.log_file, "r") as f:
    f.seek(initial_log_size)
    content = f.read()
    if f"Dispatching search for: '{sanitized_intent}'" in content:
        found_sanitized = True

if found_sanitized:
    print("✅ Success: Quotes stripped correctly.")
else:
    print("❌ FAILURE: Quotes were passed to the Oracle raw.")
    sys.exit(1)

print("\n✅ INPUT HANDLING SUITE PASSED")
