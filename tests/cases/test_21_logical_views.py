from common import MagicTest
import os
import subprocess
import time
import sys

test = MagicTest()
print("--- TEST 21: Logical Views (Tags & Inbox) ---")

# 1. Setup: Inject Data directly into DB via sudo (to bypass permission issues)
print("[Setup] Injecting 'finance' tag into DB...")
insert_sql = "INSERT INTO tags (name, color, icon) VALUES ('finance', 'green', '💰');"
cmd = ["sudo", "sqlite3", test.db_path, insert_sql]

try:
    subprocess.run(cmd, check=True)
except subprocess.CalledProcessError as e:
    print(f"❌ FAILURE: Failed to inject tag into DB: {e}")
    sys.exit(1)

# 2. Wait for Daemon to be ready
time.sleep(1) 

# 3. Verify Directory Structure
print("[Check] Verifying /magic/tags existence...")
# Note: Inode 2 is .magic (defined in HollowDrive)
tags_path = os.path.join(test.mount_point, ".magic", "tags")

if not os.path.exists(tags_path):
    print(f"❌ FAILURE: {tags_path} does not exist.")
    # We exit here because if the root doesn't exist, we can't check children
    sys.exit(1)

print("[Check] Verifying /magic/tags/finance existence...")

# We list the directory to see if 'finance' appears
try:
    items = os.listdir(tags_path)
    print(f"   Contents of tags: {items}")
    if "finance" not in items:
        print("❌ FAILURE: 'finance' tag missing from view.")
        sys.exit(1)
    print("✅ Found 'finance' tag.")
except OSError as e:
    print(f"❌ FAILURE: Could not list tags directory: {e}")
    sys.exit(1)

# 4. Verify Inbox
print("[Check] Verifying /magic/inbox existence...")
inbox_path = os.path.join(test.mount_point, ".magic", "inbox")
if not os.path.exists(inbox_path):
    print(f"❌ FAILURE: {inbox_path} does not exist.")
    sys.exit(1)

print("✅ Logical Views Verified.")
