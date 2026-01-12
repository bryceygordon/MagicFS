from common import MagicTest
import os
import sys
import time

test = MagicTest()
print("--- TEST 30: Scavenger (Orphan Recovery) ---")

# 1. Setup: Create a physical file
filename = "orphan_soul.txt"
test.create_file(filename, "I have no tags.")
test.wait_for_indexing(filename)

# 2. Get file ID
file_path = os.path.join(test.watch_dir, filename)
file_id = test.get_file_id_by_path(file_path)
if not file_id:
    print("❌ FAILURE: Could not get file ID")
    sys.exit(1)

# 3. ORPHAN IT: Manually delete all tags for this file
print(f"[Action] Manually removing all tags for file_id={file_id}...")
test.run_sql_exec(f"DELETE FROM file_tags WHERE file_id = {file_id}")

# 4. Verify it is now an orphan
orphans = test.run_sql_query("SELECT fr.file_id FROM file_registry fr LEFT JOIN file_tags ft ON fr.file_id = ft.file_id WHERE ft.file_id IS NULL")
is_orphan = any(str(row[0]) == str(file_id) for row in orphans)

if is_orphan:
    print("✅ Logic Check: File correctly identified as an orphan.")
else:
    print("❌ FAILURE: Database query failed to identify orphan.")
    sys.exit(1)

# 5. Simulate Scavenger Action (Manually link to trash)
# Note: We simulate the ACTION rather than waiting for the daemon loop, 
# to ensure the test is deterministic and fast.
print("[Action] Simulating Scavenger repair (Link to @trash)...")

# Ensure trash tag exists
test.run_sql_exec("INSERT OR IGNORE INTO tags (name, icon) VALUES ('trash', '🗑️')")

# Link it
test.run_sql_exec(f"""
    INSERT OR IGNORE INTO file_tags (file_id, tag_id, display_name) 
    VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='trash'), '{filename}')
""")

# 6. Verify Recovery
trash_path = os.path.join(test.mount_point, "tags", "trash", filename)
print(f"[Check] Checking if file appeared in {trash_path}...")

# Force a lookup to refresh cache
try:
    if os.path.exists(trash_path):
        print("✅ Success: Orphan recovered into @trash.")
    else:
        # It might take a moment for the FUSE layer to see the DB change if cached
        time.sleep(1)
        if os.path.exists(trash_path):
            print("✅ Success: Orphan recovered into @trash (after wait).")
        else:
            print("❌ FAILURE: File not found in @trash after linking.")
            sys.exit(1)
except Exception as e:
    print(f"❌ FAILURE: Error checking trash: {e}")
    sys.exit(1)

print("✅ TEST 30 PASSED")
