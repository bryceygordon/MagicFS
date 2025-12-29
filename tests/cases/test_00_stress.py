from common import MagicTest
import time
import os
import subprocess
import sqlite3

test = MagicTest()
print("--- TEST 00: STRESS & FOUNDATION AUDIT ---")

# Configuration
FILE_COUNT = 50
SUBDIR = "stress_data"

print(f"[Phase 1] Bulk Loading {FILE_COUNT} files...")
start_time = time.time()

# 1. Create 50 files
for i in range(FILE_COUNT):
    content = f"This is stress test file number {i}. The quick brown fox jumps over the lazy dog."
    test.create_file(f"{SUBDIR}/file_{i}.txt", content)

# 2. Wait for indexing
print("[Phase 1] Waiting for DB to catch up...")
def get_db_count():
    try:
        conn = sqlite3.connect(test.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT count(*) FROM file_registry")
        count = cursor.fetchone()[0]
        conn.close()
        return count
    except:
        return 0

for _ in range(60):
    count = get_db_count()
    if count >= FILE_COUNT:
        break
    time.sleep(0.5)

end_time = time.time()
final_count = get_db_count()
print(f"✅ Indexed {final_count}/{FILE_COUNT} files in {end_time - start_time:.2f}s")

# ---------------------------------------------------------
# ZOMBIE TEST
# ---------------------------------------------------------
print("\n[Phase 2] The Zombie Check (State Consistency)")
print("Deleting 10 files...")
for i in range(10):
    path = os.path.join(test.watch_dir, f"{SUBDIR}/file_{i}.txt")
    if os.path.exists(path):
        os.remove(path)

time.sleep(2) 
current_count = get_db_count()
print(f"DB Count after deletion: {current_count}")

# ---------------------------------------------------------
# THE STARTUP STORM TEST
# ---------------------------------------------------------
print("\n[Phase 3] The Startup Storm (Efficiency Audit)")

print("Stopping MagicFS daemon...")
# FIX: Use sudo pkill -x to match the process name EXACTLY 
# so we don't kill this script by accident.
subprocess.run(["sudo", "pkill", "-x", "magicfs"])
time.sleep(2)

# Delete one more while dead
zombie_path = os.path.join(test.watch_dir, f"{SUBDIR}/file_{15}.txt")
if os.path.exists(zombie_path):
    os.remove(zombie_path)

# Clear logs for clean counting
log_file = "tests/magicfs.log"
with open(log_file, "w") as f:
    f.write("")

print("Restarting MagicFS...")
binary = "./target/debug/magicfs"
cmd = ["sudo", "RUST_LOG=info", binary, test.mount_point, test.watch_dir]

with open(log_file, "a") as out:
    subprocess.Popen(cmd, stdout=out, stderr=out)

time.sleep(5) 

print("Analyzing logs for redundant operations...")
re_indexed_count = 0
with open(log_file, "r") as f:
    for line in f:
        if "[Indexer] Processing:" in line:
            re_indexed_count += 1

print(f"Files re-indexed on startup: {re_indexed_count}")
final_zombie_count = get_db_count()
print(f"Final DB Count: {final_zombie_count}")

print("\n--- RESULTS ---")
if re_indexed_count > 5:
    print(f"⚠️  EFFICIENCY FAILURE: Re-indexed {re_indexed_count} unchanged files! (The Startup Storm)")
else:
    print("✅ Efficiency Pass: Minimal re-indexing.")

expected_clean_count = FILE_COUNT - 11
if final_zombie_count > expected_clean_count:
    print(f"⚠️  CONSISTENCY FAILURE: DB has {final_zombie_count} entries, expected {expected_clean_count}. Zombies detected.")
else:
    print("✅ Consistency Pass: No zombies found.")
