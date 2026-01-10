# FILE: tests/cases/test_28_tag_moving.py
from common import MagicTest
import os
import sys
import time

test = MagicTest()
print("--- TEST 28: Tag Moving (mv folders in hierarchy) ---")

# 1. Setup: Create a hierarchy using safe transaction
print("[Setup] Creating tag hierarchy...")
setup_statements = [
    "DELETE FROM tags WHERE name IN ('finance', 'work', 'projects', 'archive', 'personal')",
    "INSERT INTO tags (name) VALUES ('finance')",
    "INSERT INTO tags (name) VALUES ('work')",
    "INSERT INTO tags (name, parent_tag_id) VALUES ('projects', (SELECT tag_id FROM tags WHERE name='work'))"
]
if not test.run_sql_transaction(setup_statements):
    print("❌ FAILURE: Could not setup tag hierarchy")
    sys.exit(1)

time.sleep(0.5)

# 2. Test moving tag to new parent (work -> finance)
print("[Action] mv /tags/work/projects /tags/finance/")
src_path = os.path.join(test.mount_point, "tags", "work", "projects")
dst_path = os.path.join(test.mount_point, "tags", "finance", "projects")

try:
    os.rename(src_path, dst_path)
    print("✅ Moved 'projects' from 'work' to 'finance'")
except Exception as e:
    print(f"❌ FAILURE: Could not move tag: {e}")
    sys.exit(1)

# 3. Verify new parent in DB using safe helper
results = test.safe_sqlite_query("""
    SELECT t1.name, t2.name as parent_name
    FROM tags t1
    JOIN tags t2 ON t1.parent_tag_id = t2.tag_id
    WHERE t1.name = 'projects'
""")
if results and results[0][1] == 'finance':
    print("✅ Database shows 'projects' parent is now 'finance'")
else:
    print(f"❌ FAILURE: Incorrect parent in DB: {results}")
    sys.exit(1)

# 4. Test renaming tag within same parent
print("[Action] mv /tags/finance/projects /tags/finance/fin_proj")
new_path = os.path.join(test.mount_point, "tags", "finance", "fin_proj")
try:
    os.rename(dst_path, new_path)
    print("✅ Renamed 'projects' to 'fin_proj' within same parent")
except Exception as e:
    print(f"❌ FAILURE: Could not rename tag: {e}")
    sys.exit(1)

# 5. Verify name change in DB using safe helper
results = test.safe_sqlite_query("SELECT name FROM tags WHERE tag_id = (SELECT tag_id FROM tags WHERE name='fin_proj')")
if results and results[0][0] == 'fin_proj':
    print("✅ Database shows renamed tag")
else:
    print(f"❌ FAILURE: Name not updated in DB: {results}")
    sys.exit(1)

# 6. Test circular dependency prevention
print("[Setup] Creating deep hierarchy...")
deep_statements = [
    "INSERT OR IGNORE INTO tags (name) VALUES ('a')",
    "INSERT OR IGNORE INTO tags (name, parent_tag_id) VALUES ('b', (SELECT tag_id FROM tags WHERE name='a'))",
    "INSERT OR IGNORE INTO tags (name, parent_tag_id) VALUES ('c', (SELECT tag_id FROM tags WHERE name='b'))"
]
if not test.run_sql_transaction(deep_statements):
    print("❌ FAILURE: Could not create deep hierarchy")
    sys.exit(1)

time.sleep(0.5)

print("[Action] Attempting to create circular dependency (c -> a)...")
src = os.path.join(test.mount_point, "tags", "a", "c")  # This doesn't exist yet
dst = os.path.join(test.mount_point, "tags", "c")      # This exists

try:
    # Try to move 'a' into 'c' which is its own descendant
    os.rename(
        os.path.join(test.mount_point, "tags", "a"),
        os.path.join(test.mount_point, "tags", "c", "a")
    )
    print("❌ FAILURE: Should prevent circular dependency")
    sys.exit(1)
except OSError as e:
    print("✅ Correctly prevented circular dependency")
except Exception as e:
    print(f"⚠️  Unexpected error: {e}")

# 7. Test moving file between tags (should still work)
print("[Setup] Creating file in 'fin_proj'...")

# Step 7.1: Clean any existing conflicting data first using safe helper
clean_statements = [
    "DELETE FROM file_tags WHERE file_id IN (SELECT file_id FROM file_registry WHERE abs_path IN ('/fake/doc.txt', '/fake_doc_moved.txt'))",
    "DELETE FROM file_registry WHERE abs_path IN ('/fake/doc.txt', '/fake_doc_moved.txt')"
]
test.run_sql_transaction(clean_statements)  # Ignore return, it's ok if no rows exist

# Step 7.2: Insert file_registry and get the actual file_id
print("  Creating file_registry entry...")
insert_result = test.safe_sqlite_execute(
    "INSERT INTO file_registry (abs_path, inode, mtime, size) VALUES ('/fake/doc.txt', 888, 1234567890, 50)"
)
if not insert_result:
    print("❌ FAILURE: Failed to create file_registry entry")
    sys.exit(1)

# Get the file_id
results = test.safe_sqlite_query("SELECT file_id FROM file_registry WHERE abs_path = '/fake/doc.txt'")
if not results:
    print("❌ FAILURE: No file_id returned from insert")
    sys.exit(1)
file_id = results[0][0]
print(f"  Created file_registry entry with file_id: {file_id}")

# Step 7.3: Create file_tags entry using the actual file_id
link_result = test.safe_sqlite_execute(
    "INSERT INTO file_tags (file_id, tag_id, display_name) VALUES (?, (SELECT tag_id FROM tags WHERE name='fin_proj'), 'doc.txt')",
    (file_id,)
)
if not link_result:
    print("❌ FAILURE: Failed to create file_tags entry")
    sys.exit(1)
print(f"  Linked file_id {file_id} to tag 'fin_proj'")

# Step 7.4: Verify the database state
verify_results = test.safe_sqlite_query("""
    SELECT t.name, ft.display_name, fr.abs_path
    FROM file_tags ft
    JOIN tags t ON ft.tag_id = t.tag_id
    JOIN file_registry fr ON ft.file_id = fr.file_id
    WHERE fr.abs_path = '/fake/doc.txt'
""")
if verify_results:
    print(f"  Database verification:")
    for row in verify_results:
        print(f"    Tag: {row[0]}, Display: {row[1]}, Path: {row[2]}")
else:
    print("⚠️  Warning: Verification query returned no results")

# Give FUSE a moment to sync
time.sleep(0.5)

print("[Action] Verifying file exists in directory...")
file_dir = os.path.join(test.mount_point, "tags", "finance", "fin_proj")
try:
    listing = os.listdir(file_dir)
    print(f"  Directory listing: {listing}")
    if "doc.txt" not in listing:
        print(f"❌ FAILURE: 'doc.txt' not found in directory listing!")
        # Debug: List what IS there
        print(f"  Full directory contents: {listing}")

        # Debug: Check if directory exists at all
        if os.path.exists(file_dir):
            print(f"  Directory exists: Yes")
        else:
            print(f"  Directory exists: No")

        sys.exit(1)
    print("✅ File visible in directory")
except Exception as e:
    print(f"❌ FAILURE: Could not list directory {file_dir}: {e}")
    sys.exit(1)

print("[Action] mv file between tags...")
file_src = os.path.join(test.mount_point, "tags", "finance", "fin_proj", "doc.txt")
file_dst = os.path.join(test.mount_point, "tags", "finance", "moved_doc.txt")

try:
    os.rename(file_src, file_dst)
    print("✅ Moved file between tags (with rename)")
except Exception as e:
    print(f"❌ FAILURE: File move failed: {e}")
    sys.exit(1)

# Verify file movement using safe helper
print(f"[Verify] Checking database for file_id={file_id}")
result = test.safe_sqlite_query("""
    SELECT ft.display_name, t.name
    FROM file_tags ft
    JOIN tags t ON ft.tag_id = t.tag_id
    WHERE ft.file_id = ?
""", (int(file_id),))

if result and result[0][0] == 'moved_doc.txt' and result[0][1] == 'finance':
    print("✅ File correctly moved to new tag with new name")
    print(f"   File is now named '{result[0][0]}' in tag '{result[0][1]}'")
else:
    print(f"❌ FAILURE: File not moved correctly: {result}")
    # Debug: Let's see ALL file_tags entries for this file
    all_tags = test.safe_sqlite_query("SELECT * FROM file_tags")
    all_files = test.safe_sqlite_query("SELECT * FROM file_registry")
    print(f"   All file_tags entries: {all_tags}")
    print(f"   All file_registry entries: {all_files}")
    sys.exit(1)

print("✅ TAG MOVING TEST PASSED")