from common import MagicTest
import time
import os
import subprocess
import sqlite3
import shutil

test = MagicTest()
print("--- TEST 00: STRESS & FOUNDATION AUDIT ---")

# Configuration
FILE_COUNT = 50
SUBDIR = "stress_data"

# Ensure clean slate for this sub-test
full_subdir_path = os.path.join(test.watch_dir, SUBDIR)
if os.path.exists(full_subdir_path):
    shutil.rmtree(full_subdir_path)
os.makedirs(full_subdir_path, exist_ok=True)

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
        result = cursor.fetchone()
        conn.close()
        return result[0] if result else 0
    except:
        return 0

# Wait loop
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

time.sleep(3) # Give Librarian time to notice deletion
current_count = get_db_count()
print(f"DB Count after deletion: {current_count}")

# ---------------------------------------------------------
# THE STARTUP STORM TEST
# ---------------------------------------------------------
print("\n[Phase 3] The Startup Storm (Efficiency Audit)")

# CRITICAL FIX: Wait for SQLite WAL to settle/flush before killing.
# If we kill too fast, the last few inserts might not be checkpointed, 
# causing them to look "new" on restart.
print("Waiting for DB consistency...")
time.sleep(3) 

print("Stopping MagicFS daemon...")
subprocess.run(["sudo", "pkill", "-x", "magicfs"])
time.sleep(2)

# Delete one more while dead to verify startup purge
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

# Launch in background
with open(log_file, "a") as out:
    subprocess.Popen(cmd, stdout=out, stderr=out)

# Wait for boot
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

# ---------------------------------------------------------
# LRU CACHE THRASHING
# ---------------------------------------------------------
print("\n[Phase 4] Cache Thrashing (LRU Verification)")
print("Generating 100 unique search queries to force eviction...")

for i in range(100):
    query = f"stress_query_{i}"
    search_path = os.path.join(test.mount_point, "search", query)
    try:
        if os.path.exists(search_path):
            pass
    except OSError:
        pass
    
    if i % 20 == 0:
        print(f"  ... sent {i} queries")

print("✅ Sent 100 queries. Daemon should still be responsive.")

# Verify responsiveness
test.create_file("lru_canary.txt", "The system is still alive")
test.wait_for_indexing("lru_canary.txt")
try:
    test.search_fs("system is still alive", "lru_canary.txt")
    print("✅ System survived cache thrashing.")
except SystemExit:
    print("❌ FAILURE: System died after cache thrashing.")
    test.dump_logs()
    exit(1)


print("\n--- RESULTS ---")
failed = False

# 1. Efficiency
if re_indexed_count > 5:
    print(f"⚠️  EFFICIENCY FAILURE: Re-indexed {re_indexed_count} unchanged files! (The Startup Storm)")
    test.dump_logs()
    failed = True
else:
    print("✅ Efficiency Pass: Minimal re-indexing.")

# 2. Consistency
expected_clean_count = FILE_COUNT - 11 + 1 
if abs(final_zombie_count - expected_clean_count) > 2:
    print(f"⚠️  CONSISTENCY FAILURE: DB has {final_zombie_count} entries, expected approx {expected_clean_count}.")
    test.dump_logs()
    failed = True
else:
    print("✅ Consistency Pass: Zombie count matches expectations.")

if failed:
    exit(1)
