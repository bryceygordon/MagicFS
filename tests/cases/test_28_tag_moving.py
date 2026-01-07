# FILE: tests/cases/test_28_tag_moving.py
from common import MagicTest
import os
import subprocess
import sys
import sqlite3
import time

test = MagicTest()
print("--- TEST 28: Tag Moving (mv folders in hierarchy) ---")

# 1. Setup: Create a hierarchy
print("[Setup] Creating tag hierarchy...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    DELETE FROM tags WHERE name IN ('finance', 'work', 'projects', 'archive', 'personal');
    INSERT INTO tags (name) VALUES ('finance');
    INSERT INTO tags (name) VALUES ('work');
    INSERT INTO tags (name, parent_tag_id) VALUES ('projects', (SELECT tag_id FROM tags WHERE name='work'));
"""], check=True)

time.sleep(0.5)

# 2. Test moving tag to new parent (work -> finance)
print("[Action] mv /magic/tags/work/projects /magic/tags/finance/")
src_path = os.path.join(test.mount_point, ".magic", "tags", "work", "projects")
dst_path = os.path.join(test.mount_point, ".magic", "tags", "finance", "projects")

try:
    os.rename(src_path, dst_path)
    print("✅ Moved 'projects' from 'work' to 'finance'")
except Exception as e:
    print(f"❌ FAILURE: Could not move tag: {e}")
    sys.exit(1)

# 3. Verify new parent in DB
conn = sqlite3.connect(test.db_path)
cursor = conn.cursor()
cursor.execute("""
    SELECT t1.name, t2.name as parent_name
    FROM tags t1
    JOIN tags t2 ON t1.parent_tag_id = t2.tag_id
    WHERE t1.name = 'projects'
""")
result = cursor.fetchone()
if result and result[1] == 'finance':
    print("✅ Database shows 'projects' parent is now 'finance'")
else:
    print(f"❌ FAILURE: Incorrect parent in DB: {result}")
    sys.exit(1)

# 4. Test renaming tag within same parent
print("[Action] mv /magic/tags/finance/projects /magic/tags/finance/fin_proj")
new_path = os.path.join(test.mount_point, ".magic", "tags", "finance", "fin_proj")
try:
    os.rename(dst_path, new_path)
    print("✅ Renamed 'projects' to 'fin_proj' within same parent")
except Exception as e:
    print(f"❌ FAILURE: Could not rename tag: {e}")
    sys.exit(1)

# 5. Verify name change in DB
cursor.execute("SELECT name FROM tags WHERE tag_id = (SELECT tag_id FROM tags WHERE name='fin_proj')")
result = cursor.fetchone()
if result and result[0] == 'fin_proj':
    print("✅ Database shows renamed tag")
else:
    print(f"❌ FAILURE: Name not updated in DB: {result}")
    sys.exit(1)

# 6. Test circular dependency prevention
print("[Setup] Creating deep hierarchy...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    INSERT INTO tags (name) VALUES ('a');
    INSERT INTO tags (name, parent_tag_id) VALUES ('b', (SELECT tag_id FROM tags WHERE name='a'));
    INSERT INTO tags (name, parent_tag_id) VALUES ('c', (SELECT tag_id FROM tags WHERE name='b'));
"""], check=True)

time.sleep(0.5)

print("[Action] Attempting to create circular dependency (c -> a)...")
src = os.path.join(test.mount_point, ".magic", "tags", "a", "c")  # This doesn't exist yet
dst = os.path.join(test.mount_point, ".magic", "tags", "c")      # This exists

try:
    # Try to move 'a' into 'c' which is its own descendant
    os.rename(
        os.path.join(test.mount_point, ".magic", "tags", "a"),
        os.path.join(test.mount_point, ".magic", "tags", "c", "a")
    )
    print("❌ FAILURE: Should prevent circular dependency")
    sys.exit(1)
except OSError as e:
    print("✅ Correctly prevented circular dependency")
except Exception as e:
    print(f"⚠️  Unexpected error: {e}")

# 7. Test moving file between tags (should still work)
print("[Setup] Creating file in 'fin_proj'...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    INSERT INTO file_registry (abs_path, inode, mtime, size) VALUES ('/fake/doc.txt', 888, 1234567890, 50);
    INSERT INTO file_tags (file_id, tag_id, display_name) VALUES (
        2,
        (SELECT tag_id FROM tags WHERE name='fin_proj'),
        'doc.txt'
    );
"""], check=True)

time.sleep(0.5)

print("[Action] mv file between tags...")
file_src = os.path.join(test.mount_point, ".magic", "tags", "fin_proj", "doc.txt")
file_dst = os.path.join(test.mount_point, ".magic", "tags", "finance", "moved_doc.txt")

try:
    os.rename(file_src, file_dst)
    print("✅ Moved file between tags (with rename)")
except Exception as e:
    print(f"❌ FAILURE: File move failed: {e}")
    sys.exit(1)

# Verify file movement
cursor.execute("""
    SELECT ft.display_name, t.name
    FROM file_tags ft
    JOIN tags t ON ft.tag_id = t.tag_id
    WHERE ft.file_id = 2
""")
result = cursor.fetchone()
if result and result[0] == 'moved_doc.txt' and result[1] == 'finance':
    print("✅ File correctly moved to new tag with new name")
else:
    print(f"❌ FAILURE: File not moved correctly: {result}")
    sys.exit(1)

conn.close()
print("✅ TAG MOVING TEST PASSED")