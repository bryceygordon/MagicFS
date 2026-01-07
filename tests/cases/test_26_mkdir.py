# FILE: tests/cases/test_26_mkdir.py
from common import MagicTest
import os
import subprocess
import sys
import sqlite3
import time

test = MagicTest()
print("--- TEST 26: mkdir (Hierarchical Tag Creation) ---")

# 1. Setup: Ensure we start with a clean tags table
print("[Setup] Cleaning up any existing test tags...")
subprocess.run(["sudo", "sqlite3", test.db_path, "DELETE FROM tags WHERE name IN ('projects', 'work', 'personal');"], check=True)

# 2. Create root-level tag via mkdir
print("[Action] mkdir /magic/tags/projects")
try:
    os.mkdir(os.path.join(test.mount_point, ".magic", "tags", "projects"))
    print("✅ Created root-level tag 'projects'")
except Exception as e:
    print(f"❌ FAILURE: Could not create root tag: {e}")
    sys.exit(1)

# 3. Verify in database
print("[Verify] Checking database...")
conn = sqlite3.connect(test.db_path)
cursor = conn.cursor()
cursor.execute("SELECT tag_id, parent_tag_id, name FROM tags WHERE name = 'projects'")
result = cursor.fetchone()
if result and result[2] == 'projects' and result[1] is None:
    print(f"✅ Tag 'projects' exists in DB with tag_id={result[0]}")
else:
    print(f"❌ FAILURE: Tag not found in DB or incorrect structure: {result}")
    sys.exit(1)

# 4. Create nested tag (child of projects)
print("[Action] mkdir /magic/tags/projects/work")
try:
    os.mkdir(os.path.join(test.mount_point, ".magic", "tags", "projects", "work"))
    print("✅ Created nested tag 'work' under 'projects'")
except Exception as e:
    print(f"❌ FAILURE: Could not create nested tag: {e}")
    sys.exit(1)

# 5. Verify nested structure
cursor.execute("""
    SELECT t1.tag_id, t1.parent_tag_id, t1.name, t2.tag_id as parent_id
    FROM tags t1
    LEFT JOIN tags t2 ON t1.parent_tag_id = t2.tag_id
    WHERE t1.name = 'work'
""")
result = cursor.fetchone()
if result and result[2] == 'work' and result[1] is not None and result[3] is not None:
    print(f"✅ Nested tag 'work' exists with parent_id={result[1]}")
else:
    print(f"❌ FAILURE: Nested tag structure incorrect: {result}")
    sys.exit(1)

# 6. Test duplicate prevention
print("[Action] Attempting to create duplicate tag 'projects'...")
try:
    os.mkdir(os.path.join(test.mount_point, ".magic", "tags", "projects"))
    print("❌ FAILURE: Should not allow duplicate tag names")
    sys.exit(1)
except FileExistsError:
    print("✅ Correctly prevented duplicate tag creation")
except Exception as e:
    print(f"❌ FAILURE: Wrong error type: {e}")
    sys.exit(1)

# 7. Test creating tag with same name under different parent
print("[Action] Creating 'projects' under different parent...")
try:
    # First create a different parent
    os.mkdir(os.path.join(test.mount_point, ".magic", "tags", "personal"))
    # Now create 'projects' under 'personal' (should work)
    os.mkdir(os.path.join(test.mount_point, ".magic", "tags", "personal", "projects"))
    print("✅ Successfully created 'personal/projects' (different namespace)")
except Exception as e:
    print(f"❌ FAILURE: Should allow same name under different parent: {e}")
    sys.exit(1)

# 8. Verify all tags in DB
cursor.execute("SELECT name, parent_tag_id FROM tags ORDER BY tag_id")
all_tags = cursor.fetchall()
print(f"[Info] All tags in DB: {all_tags}")

conn.close()
print("✅ MKDIR TEST PASSED")