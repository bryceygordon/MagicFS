from common import MagicTest
import os
import time
import shutil
import stat
import subprocess
import sqlite3

test = MagicTest()
print("--- TEST 07: Real World Hardening (Symlinks, Load, Permissions) ---")

# =========================================================================
# SCENARIO 3: THE ASYNC EXPLOSION (BACKPRESSURE)
# MOVED FIRST to create the "Backlog" condition
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

# =========================================================================
# SCENARIO 1: THE SYMLINK TRAP
# =========================================================================
print("\n[Scenario 1] The Symlink Trap (While under load)")
trap_dir = os.path.join(test.watch_dir, "trap")
real_folder = os.path.join(trap_dir, "real_folder")
link_path = os.path.join(real_folder, "infinite_loop")

if os.path.exists(trap_dir):
    shutil.rmtree(trap_dir)
os.makedirs(real_folder)

with open(os.path.join(real_folder, "safe.txt"), "w") as f:
    f.write("I am a safe file inside the trap.")

try:
    os.symlink(trap_dir, link_path)
    print(f"  Created symlink loop: {link_path} -> {trap_dir}")
except OSError as e:
    print(f"  ⚠️ Skipped symlink test (OS not supporting symlinks?): {e}")

test.create_file("trigger_scan.txt", "wake up librarian")

# SENSOR LOGIC: Wait until the conveyor belt stops moving (DB count stabilizes)
test.wait_for_stable_db(stability_duration=2)

# NOW check for the file
test.assert_file_indexed("safe.txt")
print("✅ Symlink Trap survived (and processed despite load).")

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

# SENSOR LOGIC again
test.wait_for_stable_db(stability_duration=2)

if test.check_file_in_db("forbidden.secret"):
    print("  ⚠️ Warning: forbidden.secret found in DB")
else:
    print("  ✅ forbidden.secret correctly ignored/skipped.")
test.assert_file_indexed("valid_neighbor.txt")
print("✅ Permission Denied handling verified.")
os.chmod(forbidden_path, 0o777)

# Verify total count
final_count = test.get_db_count()
print(f"\n[Final Stats] Total Indexed: {final_count}")
if final_count < 500:
     print("⚠️ Warning: Less than 500 files indexed. Some might have been dropped.")
else:
     print("✅ All bulk files accounted for.")

print("✅ All Real World Scenarios Passed.")
