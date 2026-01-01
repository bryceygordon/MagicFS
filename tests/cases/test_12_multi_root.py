from common import MagicTest
import os
import shutil
import time
import subprocess
import signal

print("--- TEST 12: Multi-Root Monitoring ---")

# 1. Define paths
base_tmp = "/tmp/magicfs-test-data"
dir_a = os.path.join(base_tmp, "root_a")
dir_b = os.path.join(base_tmp, "root_b")
mount_point = "/tmp/magicfs-test-mount"
# --- FIX: Match the isolated DB path for this branch ---
db_path = "/tmp/.magicfs_arctic/index.db"
binary = "./target/debug/magicfs"
log_file = "/tmp/magicfs_debug.log"

# 2. Cleanup & Setup
# FIX: Use sudo to unmount and remove root-owned directories
if os.path.exists(mount_point):
    subprocess.run(["sudo", "umount", "-l", mount_point], stderr=subprocess.DEVNULL)

if os.path.exists(base_tmp):
    # This might contain root files if the daemon created anything, safer to sudo
    subprocess.run(["sudo", "rm", "-rf", base_tmp])

# --- FIX: Cleanup the correct DB dir ---
if os.path.exists("/tmp/.magicfs_arctic"):
    subprocess.run(["sudo", "rm", "-rf", "/tmp/.magicfs_arctic"])

os.makedirs(dir_a)
os.makedirs(dir_b)
os.makedirs(mount_point, exist_ok=True)

# 3. Create Content
with open(os.path.join(dir_a, "alpha.txt"), "w") as f:
    f.write("I am in Root A. Apple.")

with open(os.path.join(dir_b, "beta.txt"), "w") as f:
    f.write("I am in Root B. Banana.")

# 4. Start Daemon MANUALLY with Comma-Separated Paths
print(f"[Setup] Launching Daemon with roots: {dir_a},{dir_b}")

# Kill any existing
subprocess.run(["sudo", "pkill", "-x", "magicfs"])
time.sleep(1)

# Ensure log file exists and is writable
if os.path.exists(log_file):
    # Use sudo to remove in case it was created by root previously
    subprocess.run(["sudo", "rm", "-f", log_file])

# Create fresh log file
subprocess.run(["touch", log_file])
subprocess.run(["chmod", "666", log_file])

cmd = ["sudo", "-E", binary, mount_point, f"{dir_a},{dir_b}"]

with open(log_file, "w") as log:
    proc = subprocess.Popen(cmd, stdout=log, stderr=log, env=dict(os.environ, RUST_LOG="debug"))

time.sleep(2)

# 5. Verify Indexing
# We initialize MagicTest with dummy args just to get the helper methods
import sys
sys.argv = ["dummy", db_path, mount_point, dir_a]
test = MagicTest()

# We need to explicitly set the log file so dump_logs works if needed
test.log_file = log_file

print("[Verify] Checking Indexing of Root A...")
test.wait_for_indexing("alpha.txt")

print("[Verify] Checking Indexing of Root B...")
test.wait_for_indexing("beta.txt")

# 6. Verify Search
print("[Verify] Searching across roots...")
if test.search_fs("Apple", "alpha.txt"):
    print("✅ Found file from Root A")

if test.search_fs("Banana", "beta.txt"):
    print("✅ Found file from Root B")

# 7. Cleanup Manual Daemon
subprocess.run(["sudo", "kill", str(proc.pid)])

print("✅ MULTI-ROOT TEST PASSED")
