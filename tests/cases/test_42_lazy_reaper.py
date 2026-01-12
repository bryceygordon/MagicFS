from common import MagicTest
import os
import sys
import time
import subprocess

test = MagicTest()
print("--- TEST 42: The Lazy Reaper (Ghost Cleanup) ---")

# 1. Setup: Create a tag 'ghostbusters'
print("[Setup] Injecting 'ghostbusters' tag...")
test.run_sql_exec("INSERT OR IGNORE INTO tags (name, color) VALUES ('ghostbusters', '#555555')")

# 2. Inject a Ghost Record
# We create a DB entry for a file that definitely does not exist on disk.
fake_path = os.path.join(test.watch_dir, "phantom_file.txt")
if os.path.exists(fake_path):
    os.remove(fake_path)

print(f"[Setup] Creating ghost record for: {fake_path}")

# Insert into registry
sql_reg = f"INSERT INTO file_registry (abs_path, inode, mtime, size) VALUES ('{fake_path}', 999999, 123456, 1024)"
test.run_sql_exec(sql_reg)

# Get the ID we just made
file_id = test.get_file_id_by_path(fake_path)
if not file_id:
    print("❌ FAILURE: Failed to inject ghost record.")
    sys.exit(1)

# Link to tag
sql_link = f"INSERT INTO file_tags (file_id, tag_id, display_name) VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='ghostbusters'), 'phantom_file.txt')"
test.run_sql_exec(sql_link)

print("✅ Ghost injected into DB.")

# 3. Verify the Ghost is currently "visible" to the DB
# (Sanity check our injection worked)
if not test.check_file_in_db("phantom_file.txt"):
    print("❌ FAILURE: DB injection didn't stick.")
    sys.exit(1)

# 4. Trigger the Reaper (List the directory)
# This `ls` should trigger readdir(), which should check existence, fail, and trigger cleanup.
target_dir = os.path.join(test.mount_point, "tags", "ghostbusters")
print(f"[Action] Listing {target_dir}...")

try:
    items = os.listdir(target_dir)
    print(f"  Contents: {items}")
    
    # CHECK 1: Visibility
    if "phantom_file.txt" in items:
        print("❌ FAILURE: Ghost file is visible in FUSE listing! (Reaper didn't filter it)")
        sys.exit(1)
    else:
        print("✅ Success: Ghost file hidden from user.")

except OSError as e:
    print(f"❌ FAILURE: Listing failed: {e}")
    sys.exit(1)

# 5. Verify Cleanup Persistence
# The cleanup might be async, so we give it a moment
print("[Check] Verifying database purge...")
time.sleep(1.0)

if test.check_file_in_db("phantom_file.txt"):
    print("❌ FAILURE: Ghost record still exists in DB! (Reaper didn't delete it)")
    sys.exit(1)
else:
    print("✅ Success: Ghost record purged from database.")

print("✅ LAZY REAPER TEST PASSED")
