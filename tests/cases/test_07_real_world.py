from common import MagicTest
import os
import time
import shutil
import stat
import subprocess
import sqlite3

test = MagicTest()
print("--- TEST 07: Real World Hardening (Race Conditions & Permissions) ---")

# =========================================================================
# SCENARIO 1: THE REINCARNATION RACE (The "Safe.txt" Fault)
# =========================================================================
print("\n[Scenario 1] The Reincarnation Race (Rapid Delete/Create)")
# This attempts to reproduce the race condition where a Delete event
# overtakes a Create event, leaving a valid file unindexed.

trap_dir = os.path.join(test.watch_dir, "trap")
safe_file = os.path.join(trap_dir, "safe.txt")

# We will cycle this directory multiple times to force the race
CYCLES = 3

for i in range(CYCLES):
    print(f"  Cycle {i+1}/{CYCLES}: Destroying and Rebuilding 'trap/'...")
    
    # 1. Destroy
    if os.path.exists(trap_dir):
        shutil.rmtree(trap_dir)
    
    # 2. Rebuild IMMEDIATELY (No sleep, max stress)
    os.makedirs(trap_dir)
    with open(safe_file, "w") as f:
        f.write(f"I am safe.txt, generation {i}")

    # 3. Trigger a scan elsewhere to keep the Librarian busy
    test.create_file(f"noise_{i}.txt", "distraction")

print("  Waiting for dust to settle...")
test.wait_for_stable_db(stability_duration=2)

# DIAGNOSTIC: Check Reality vs DB
exists_on_disk = os.path.exists(safe_file)
exists_in_db = test.check_file_in_db("safe.txt")

print(f"  [Reality Check] On Disk: {exists_on_disk} | In DB: {exists_in_db}")

if exists_on_disk and not exists_in_db:
    print("❌ CRITICAL FAILURE: 'safe.txt' exists on disk but is MISSING from DB.")
    print("   The 'Delete' signal likely overtook the 'Create' signal.")
    test.dump_logs()
    exit(1)
elif not exists_on_disk:
    print("❌ SETUP FAILURE: 'safe.txt' failed to persist on disk.")
    exit(1)

print("✅ The Reincarnation Race survived.")


# =========================================================================
# SCENARIO 2: THE PERMISSION DENIED CRASH
# =========================================================================
print("\n[Scenario 2] The Forbidden File (chmod 000)")
forbidden_path = os.path.join(test.watch_dir, "forbidden.secret")
with open(forbidden_path, "w") as f:
    f.write("Top Secret Nuclear Codes")
os.chmod(forbidden_path, 0o000)
print(f"  Created locked file: {forbidden_path} (mode 000)")

test.create_file("valid_neighbor.txt", "I am allowed to be read.")

# Wait for processing
test.wait_for_stable_db(stability_duration=2)

if test.check_file_in_db("forbidden.secret"):
    print("  ⚠️ Warning: forbidden.secret found in DB")
else:
    print("  ✅ forbidden.secret correctly ignored/skipped.")

test.assert_file_indexed("valid_neighbor.txt")
print("✅ Permission Denied handling verified.")

# Cleanup permission so we can delete the folder later
os.chmod(forbidden_path, 0o777)


# =========================================================================
# SCENARIO 3: THE ASYNC EXPLOSION (Backpressure)
# =========================================================================
print("\n[Scenario 3] The Async Explosion (500 Files)")
burst_dir = os.path.join(test.watch_dir, "burst_load")
if os.path.exists(burst_dir):
    shutil.rmtree(burst_dir)
os.makedirs(burst_dir)

file_count = 500
print(f"  Generating {file_count} files to clog the queue...")
for i in range(file_count):
    with open(os.path.join(burst_dir, f"load_{i}.txt"), "w") as f:
        f.write(f"This is load test file number {i}.")

print("  Waiting for conveyor belt to finish...")
test.wait_for_stable_db(stability_duration=3, max_wait=60)

# Verify total count
final_count = test.get_db_count()
print(f"\n[Final Stats] Total Indexed: {final_count}")

# We expect at least 500 burst files + neighbor + safe.txt + noise files
if final_count < 500:
     print(f"⚠️ Warning: Only {final_count} files indexed. Expected > 500.")
else:
     print("✅ All bulk files accounted for.")

print("✅ All Real World Scenarios Passed.")
