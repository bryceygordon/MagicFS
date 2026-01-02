from common import MagicTest
import os
import shutil
import time
import subprocess

print("--- TEST 14: Mirror Mode Navigation ---")

# Setup Paths
base_tmp = "/tmp/magicfs-test-data"
root_dir = os.path.join(base_tmp, "my_docs")
sub_dir = os.path.join(root_dir, "projects")
mount_point = "/tmp/magicfs-test-mount"
# UPDATED PATH: Matched to main.rs (Nomic)
db_path = "/tmp/.magicfs_nomic/index.db"
binary = "./target/debug/magicfs"
log_file = "/tmp/magicfs_debug.log"

# Cleanup
if os.path.exists(mount_point):
    subprocess.run(["sudo", "umount", "-l", mount_point], stderr=subprocess.DEVNULL)
if os.path.exists(base_tmp):
    subprocess.run(["sudo", "rm", "-rf", base_tmp])
# CLEANUP CORRECT DB DIR
if os.path.exists("/tmp/.magicfs_nomic"):
    subprocess.run(["sudo", "rm", "-rf", "/tmp/.magicfs_nomic"])

os.makedirs(sub_dir)
os.makedirs(mount_point, exist_ok=True)

# Create Content
file_path = os.path.join(sub_dir, "notes.txt")
with open(file_path, "w") as f:
    f.write("Original Content")

# Launch Daemon
subprocess.run(["sudo", "pkill", "-x", "magicfs"])
time.sleep(1)

if os.path.exists(log_file): subprocess.run(["sudo", "rm", "-f", log_file])
subprocess.run(["touch", log_file])
subprocess.run(["chmod", "666", log_file])

print(f"[Setup] Mounting {root_dir}...")
cmd = ["sudo", "-E", binary, mount_point, root_dir]
with open(log_file, "w") as log:
    proc = subprocess.Popen(cmd, stdout=log, stderr=log, env=dict(os.environ, RUST_LOG="debug"))

time.sleep(2)

# Verify Mirror Structure
mirror_root = os.path.join(mount_point, "mirror")
if not os.path.exists(mirror_root):
    print("❌ FAILURE: /mirror directory missing!")
    exit(1)

visible_roots = os.listdir(mirror_root)
print(f"[Verify] Mirror Roots: {visible_roots}")
if "my_docs" not in visible_roots:
    print("❌ FAILURE: 'my_docs' missing from /mirror")
    exit(1)

# Navigate Deep
mirror_file = os.path.join(mirror_root, "my_docs", "projects", "notes.txt")
print(f"[Verify] Checking file: {mirror_file}")

if not os.path.exists(mirror_file):
    print("❌ FAILURE: Deep navigation failed. File not found.")
    exit(1)

# Read
with open(mirror_file, "r") as f:
    content = f.read()
    if content != "Original Content":
        print("❌ FAILURE: Read mismatch.")
        exit(1)
    print("✅ Read Successful.")

# Write
with open(mirror_file, "w") as f:
    f.write("Updated via Mirror")

# Verify Real Disk
with open(file_path, "r") as f:
    real_content = f.read()
    if real_content != "Updated via Mirror":
        print("❌ FAILURE: Write did not persist to disk.")
        exit(1)
    print("✅ Write Successful.")

subprocess.run(["sudo", "kill", str(proc.pid)])
print("✅ MIRROR MODE PASSED")
