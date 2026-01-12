from common import MagicTest
import os
import sys
import time

test = MagicTest()
print("--- TEST 29: Wastebin (Soft Delete) ---")

# 1. Setup: Create file and tag it
filename = "important_doc.txt"
test.create_file(filename, "Do not delete me physically.")
test.wait_for_indexing(filename)

# Create tag 'projects'
test.run_sql_exec("INSERT OR IGNORE INTO tags (name) VALUES ('projects')")

# Link file to 'projects'
file_path = os.path.join(test.watch_dir, filename)
file_id = test.get_file_id_by_path(file_path)
test.run_sql_exec(f"""
    INSERT OR IGNORE INTO file_tags (file_id, tag_id, display_name) 
    VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='projects'), '{filename}')
""")

# 2. Verify visibility in tag
virtual_path = os.path.join(test.mount_point, "tags", "projects", filename)
if not os.path.exists(virtual_path):
    print("❌ FAILURE: Setup failed, file not visible in tag.")
    sys.exit(1)

# 3. PERFORM DELETE (Soft Delete)
print(f"[Action] Deleting {virtual_path}...")
try:
    os.remove(virtual_path)
except OSError as e:
    print(f"❌ FAILURE: `os.remove` failed: {e}")
    sys.exit(1)

# 4. Verify: Gone from View
if os.path.exists(virtual_path):
    print("❌ FAILURE: File still visible in tag after delete!")
    sys.exit(1)
print("✅ File removed from Tag View.")

# 5. Verify: Still in Registry (Safety Check)
registry_check = test.run_sql_query(f"SELECT file_id FROM file_registry WHERE file_id = {file_id}")
if not registry_check:
    print("❌ FAILURE: File removed from registry! (Data Loss Risk)")
    sys.exit(1)
print("✅ File record persists in registry.")

# 6. Verify: Physical File Exists
if os.path.exists(file_path):
    print("✅ Physical file still exists on disk.")
else:
    print("❌ FAILURE: Physical file deleted! (Hard Delete occurred)")
    sys.exit(1)

# 7. Verify: Orphaned (Ready for Scavenger)
# It should now have 0 tags.
tags_count = test.run_sql_query(f"SELECT COUNT(*) FROM file_tags WHERE file_id = {file_id}")
count = int(tags_count[0][0])

if count == 0:
    print("✅ File is now an orphan (0 tags).")
else:
    print(f"❌ FAILURE: File still has {count} tags.")
    sys.exit(1)

print("✅ TEST 29 PASSED")
