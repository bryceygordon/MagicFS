# FILE: tests/cases/test_26_mkdir.py
from common import MagicTest
import os
import sys
import time

test = MagicTest()
print("--- TEST 26: mkdir (Hierarchical Tag Creation) ---")

# 1. Setup: Ensure we start with a clean tags table using safe transaction
print("[Setup] Cleaning up any existing test tags...")
if not test.run_sql_transaction(["DELETE FROM tags WHERE name IN ('projects', 'work', 'personal')"]):
    print("❌ FAILURE: Could not clean up existing tags")
    sys.exit(1)

# 2. Create root-level tag via mkdir
print("[Action] mkdir /magic/tags/projects")
try:
    os.mkdir(os.path.join(test.mount_point, "tags", "projects"))
    print("✅ Created root-level tag 'projects'")
except Exception as e:
    print(f"❌ FAILURE: Could not create root tag: {e}")
    sys.exit(1)

# 3. Verify in database using safe helper
print("[Verify] Checking database...")
results = test.safe_sqlite_query("SELECT tag_id, parent_tag_id, name FROM tags WHERE name = 'projects'")
if results:
    row = results[0]
    if len(row) >= 3 and row[2] == 'projects' and (row[1] is None or row[1] == ''):
        print(f"✅ Tag 'projects' exists in DB with tag_id={row[0]}")
    else:
        print(f"❌ FAILURE: Tag not found in DB or incorrect structure: {row}")
        sys.exit(1)
else:
    print("❌ FAILURE: Tag 'projects' not found in DB")
    sys.exit(1)

# 4. Create nested tag (child of projects)
print("[Action] mkdir /magic/tags/projects/work")
try:
    os.mkdir(os.path.join(test.mount_point, "tags", "projects", "work"))
    print("✅ Created nested tag 'work' under 'projects'")
except Exception as e:
    print(f"❌ FAILURE: Could not create nested tag: {e}")
    sys.exit(1)

# 5. Verify nested structure using safe helper
results = test.safe_sqlite_query("""
    SELECT t1.tag_id, t1.parent_tag_id, t1.name, t2.tag_id as parent_id
    FROM tags t1
    LEFT JOIN tags t2 ON t1.parent_tag_id = t2.tag_id
    WHERE t1.name = 'work'
""")
if results:
    row = results[0]
    if len(row) >= 4 and row[2] == 'work' and row[1] not in (None, '') and row[3] not in (None, ''):
        print(f"✅ Nested tag 'work' exists with parent_id={row[1]}")
    else:
        print(f"❌ FAILURE: Nested tag structure incorrect: {row}")
        sys.exit(1)
else:
    print("❌ FAILURE: Nested tag 'work' not found")
    sys.exit(1)

# 6. Test duplicate prevention
print("[Action] Attempting to create duplicate tag 'projects'...")
try:
    os.mkdir(os.path.join(test.mount_point, "tags", "projects"))
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
    os.mkdir(os.path.join(test.mount_point, "tags", "personal"))
    # Now create 'projects' under 'personal' (should work)
    os.mkdir(os.path.join(test.mount_point, "tags", "personal", "projects"))
    print("✅ Successfully created 'personal/projects' (different namespace)")
except Exception as e:
    print(f"❌ FAILURE: Should allow same name under different parent: {e}")
    sys.exit(1)

# 8. Verify all tags in DB using safe helper
results = test.safe_sqlite_query("SELECT name, parent_tag_id FROM tags ORDER BY tag_id")
all_tags = []
for row in results:
    name = row[0]
    parent = row[1] if row[1] else 'NULL'
    all_tags.append((name, parent))
print(f"[Info] All tags in DB: {all_tags}")

print("✅ MKDIR TEST PASSED")