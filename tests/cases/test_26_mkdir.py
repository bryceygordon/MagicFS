# FILE: tests/cases/test_26_mkdir.py
from common import MagicTest
import os
import subprocess
import sys
import time

test = MagicTest()
print("--- TEST 26: mkdir (Hierarchical Tag Creation) ---")

# 1. Setup: Ensure we start with a clean tags table
print("[Setup] Cleaning up any existing test tags...")
subprocess.run(["sudo", "sqlite3", test.db_path, "DELETE FROM tags WHERE name IN ('projects', 'work', 'personal');"], check=True)

# 2. Create root-level tag via mkdir
print("[Action] mkdir /magic/tags/projects")
try:
    os.mkdir(os.path.join(test.mount_point, "tags", "projects"))
    print("✅ Created root-level tag 'projects'")
except Exception as e:
    print(f"❌ FAILURE: Could not create root tag: {e}")
    sys.exit(1)

# 3. Verify in database
print("[Verify] Checking database...")
cmd = ["sudo", "sqlite3", test.db_path, "SELECT tag_id, parent_tag_id, name FROM tags WHERE name = 'projects'"]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
output = result.stdout.strip()
if output:
    parts = output.split('|')
    if len(parts) >= 3 and parts[2] == 'projects' and parts[1] == '':
        print(f"✅ Tag 'projects' exists in DB with tag_id={parts[0]}")
    else:
        print(f"❌ FAILURE: Tag not found in DB or incorrect structure: {output}")
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

# 5. Verify nested structure
cmd = ["sudo", "sqlite3", test.db_path, """
    SELECT t1.tag_id, t1.parent_tag_id, t1.name, t2.tag_id as parent_id
    FROM tags t1
    LEFT JOIN tags t2 ON t1.parent_tag_id = t2.tag_id
    WHERE t1.name = 'work'
"""]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
output = result.stdout.strip()
if output:
    parts = output.split('|')
    if len(parts) >= 4 and parts[2] == 'work' and parts[1] != '' and parts[3] != '':
        print(f"✅ Nested tag 'work' exists with parent_id={parts[1]}")
    else:
        print(f"❌ FAILURE: Nested tag structure incorrect: {output}")
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

# 8. Verify all tags in DB
cmd = ["sudo", "sqlite3", test.db_path, "SELECT name, parent_tag_id FROM tags ORDER BY tag_id"]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
all_tags = []
for line in result.stdout.strip().split('\n'):
    if line.strip():
        parts = line.split('|')
        if len(parts) >= 2:
            name = parts[0]
            parent = parts[1] if parts[1] else 'NULL'
            all_tags.append((name, parent))
print(f"[Info] All tags in DB: {all_tags}")

print("✅ MKDIR TEST PASSED")