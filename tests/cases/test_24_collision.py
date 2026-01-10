# FILE: tests/cases/test_24_collision.py
from common import MagicTest
import os
import sys
import time

test = MagicTest()
print("--- TEST 24: Collision Resolution (The Doppelgänger) ---")

# 1. Setup: Create two files with the SAME name in DIFFERENT directories
dir_a = os.path.join(test.watch_dir, "folder_A")
dir_b = os.path.join(test.watch_dir, "folder_B")
os.makedirs(dir_a, exist_ok=True)
os.makedirs(dir_b, exist_ok=True)

# FIX: Wait for watcher to attach to new directories
time.sleep(1.0)

filename = "report.pdf"
path_a = os.path.join(dir_a, filename)
path_b = os.path.join(dir_b, filename)

# FIX: Content length > 10 to bypass Indexer Noise Filter
with open(path_a, "w") as f: f.write("This is substantial Content A")
with open(path_b, "w") as f: f.write("This is substantial Content B")

# 2. Index them
test.wait_for_indexing("folder_A/report.pdf")
test.wait_for_indexing("folder_B/report.pdf")

# 3. Get IDs using the new helper method
id_a = test.get_file_id_by_path(path_a)
id_b = test.get_file_id_by_path(path_b)

if id_a is None or id_b is None:
    print("❌ FAILURE: Could not retrieve file IDs")
    test.dump_logs()
    sys.exit(1)

# 4. Inject Tag 'work' and link BOTH files
print("[Setup] Tagging both files as 'work'...")
setup_sql = f"""
INSERT INTO tags (name) VALUES ('work');
INSERT INTO file_tags (file_id, tag_id, display_name) VALUES ({id_a}, (SELECT tag_id FROM tags WHERE name='work'), '{filename}');
INSERT INTO file_tags (file_id, tag_id, display_name) VALUES ({id_b}, (SELECT tag_id FROM tags WHERE name='work'), '{filename}');
"""
success = test.run_sql_exec(setup_sql)
if not success:
    print("❌ FAILURE: Could not inject tags")
    test.dump_logs()
    sys.exit(1)

# 5. List the Tag View
view_path = os.path.join(test.mount_point, "tags", "work")
print(f"[Action] Listing {view_path}...")

try:
    items = os.listdir(view_path)
    print(f"  Contents: {items}")
    
    # We expect 2 items. 
    if len(items) != 2:
        print(f"❌ FAILURE: Expected 2 items, got {len(items)}. Collision resolution missing.")
        sys.exit(1)
        
    if len(set(items)) != 2:
        print("❌ FAILURE: Items are not unique strings.")
        sys.exit(1)

    print("✅ Successfully resolved collision.")

except OSError as e:
    print(f"❌ FAILURE: OS Error: {e}")
    sys.exit(1)
