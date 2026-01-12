from common import MagicTest
import os
import time
import subprocess
import shutil
import sys

test = MagicTest()
print("--- TEST 32: Broken Link Detection ---")

print("\n=== SCENARIO A: Offline Deletion Detection ===")
filename = "offline_test.txt"
test.create_file(filename, "I will be deleted while you sleep.")
test.wait_for_indexing(filename)

# Verify initial state
count = test.get_db_count()
if count < 1:
    print("❌ Setup failed: DB count 0")
    sys.exit(1)
print(f"✅ File indexed (DB count: {count})")

# 1. STOP DAEMON
print("🛑 Stopping daemon (simulating offline)...")
subprocess.run(["sudo", "pkill", "-x", "magicfs"], check=False)
time.sleep(2)

# 2. FORCE UNMOUNT (Crucial Fix for os error 107)
# The previous daemon left the mountpoint in a zombie state.
print("🔌 Force unmounting to prevent Zombie Mounts...")
subprocess.run(["sudo", "umount", "-l", test.mount_point], stderr=subprocess.DEVNULL)
time.sleep(1)

# 3. DELETE FILE PHYSICALLY
file_path = os.path.join(test.watch_dir, filename)
if os.path.exists(file_path):
    os.remove(file_path)
    print(f"🗑️  Physically deleted: {file_path}")
else:
    print("❌ Setup failed: File missing before deletion")
    sys.exit(1)

# 4. RESTART DAEMON
print("🚀 Restarting daemon...")
binary = "./target/debug/magicfs"
log_file = os.environ.get("MAGICFS_LOG_FILE", "/tmp/magicfs_debug.log")

# Ensure mountpoint exists
os.makedirs(test.mount_point, exist_ok=True)

with open(log_file, "a") as log:
    # Use -E to preserve RUST_LOG environment variable
    proc = subprocess.Popen(
        ["sudo", "-E", binary, test.mount_point, test.watch_dir],
        stdout=log,
        stderr=log
    )

# Wait for startup and War Mode scan
print("⏳ Waiting for War Mode scan...")
time.sleep(5) 

# 5. VERIFY CLEANUP
# The startup scan should have detected the missing file and removed it
print("[Sensor] Checking DB for cleanup...")
final_count = test.get_db_count()

if final_count == 0:
    print("✅ SUCCESS: Orphan record cleaned up on startup.")
else:
    print(f"❌ FAILURE: Orphan record persisted (count: {final_count})")
    # Debug info
    print("   Check logs to see if 'War Mode' ran or if FUSE failed.")
    subprocess.run(["tail", "-n", "20", log_file])
    
    # Cleanup
    subprocess.run(["sudo", "kill", str(proc.pid)], check=False)
    sys.exit(1)

# Cleanup process
subprocess.run(["sudo", "kill", str(proc.pid)], check=False)
print("✅ TEST 32 PASSED")
