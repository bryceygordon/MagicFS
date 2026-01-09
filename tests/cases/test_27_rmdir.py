# FILE: tests/cases/test_27_rmdir.py
from common import MagicTest
import os
import subprocess
import sys
import time

test = MagicTest()
print("--- TEST 27: rmdir (Hierarchical Tag Deletion) ---")

# 1. Setup: Create a hierarchy for testing
print("[Setup] Creating test hierarchy...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    DELETE FROM tags WHERE name IN ('empty', 'parent', 'child1', 'child2', 'withfile');
    INSERT INTO tags (name) VALUES ('empty');
"""], check=True)

# Mount refresh to pick up the new tag
# 2. Verify empty tag exists in filesystem
print("[Verify] Checking empty tag exists...")
empty_path = os.path.join(test.mount_point, "tags", "empty")
if os.path.exists(empty_path):
    print("✅ Empty tag exists in mount")
else:
    print("❌ FAILURE: Empty tag not visible")
    sys.exit(1)

# 3. Test deleting empty tag
print("[Action] rmdir empty tag...")
try:
    os.rmdir(empty_path)
    print("✅ Successfully removed empty tag")
except Exception as e:
    print(f"❌ FAILURE: Could not remove empty tag: {e}")
    sys.exit(1)

# 4. Verify tag is gone from DB
cmd = ["sudo", "sqlite3", test.db_path, "SELECT COUNT(*) FROM tags WHERE name = 'empty'"]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
count = int(result.stdout.strip())
if count == 0:
    print("✅ Tag removed from database")
else:
    print(f"❌ FAILURE: Tag still exists in DB (count={count})")
    sys.exit(1)

# 5. Test deleting non-empty tag (should fail)
print("[Setup] Creating tag with file...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    INSERT INTO tags (name) VALUES ('withfile');
    INSERT INTO file_registry (abs_path, inode, mtime, size) VALUES ('/fake/path/file.txt', 999, 1234567890, 100);
    INSERT INTO file_tags (file_id, tag_id) VALUES (1, (SELECT tag_id FROM tags WHERE name='withfile'));
"""], check=True)

# Refresh mount
time.sleep(0.5)

print("[Action] Attempting to remove non-empty tag...")
nonempty_path = os.path.join(test.mount_point, "tags", "withfile")
try:
    os.rmdir(nonempty_path)
    print("❌ FAILURE: Should not remove tag containing files")
    sys.exit(1)
except OSError:
    print("✅ Correctly prevented deletion of non-empty tag")
except Exception as e:
    print(f"❌ FAILURE: Wrong error type: {e}")
    sys.exit(1)

# 6. Test deleting tag with children (should fail)
print("[Setup] Creating parent-child hierarchy...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    INSERT INTO tags (name) VALUES ('parent');
    INSERT INTO tags (name, parent_tag_id) VALUES ('child1', (SELECT tag_id FROM tags WHERE name='parent'));
    INSERT INTO tags (name, parent_tag_id) VALUES ('child2', (SELECT tag_id FROM tags WHERE name='parent'));
"""], check=True)

time.sleep(0.5)

print("[Action] Attempting to remove parent with children...")
parent_path = os.path.join(test.mount_point, "tags", "parent")
try:
    os.rmdir(parent_path)
    print("❌ FAILURE: Should not remove tag containing children")
    sys.exit(1)
except OSError:
    print("✅ Correctly prevented deletion of parent tag")
except Exception as e:
    print(f"❌ FAILURE: Wrong error type: {e}")
    sys.exit(1)

# 7. Test deleting nested child after removing other children
print("[Setup] Remove one child...")
subprocess.run(["sudo", "sqlite3", test.db_path, """
    DELETE FROM tags WHERE name = 'child1';
"""], check=True)

# Remove parent should still fail because child2 exists
print("[Action] Attempting to remove parent with one remaining child...")
try:
    os.rmdir(parent_path)
    print("❌ FAILURE: Should not remove parent with remaining children")
    sys.exit(1)
except OSError:
    print("✅ Still correctly prevented deletion")

# 8. Remove the other child and then parent should work
subprocess.run(["sudo", "sqlite3", test.db_path, """
    DELETE FROM tags WHERE name = 'child2';
"""], check=True)

time.sleep(0.5)

print("[Action] Removing parent after all children deleted...")
try:
    os.rmdir(parent_path)
    print("✅ Successfully removed parent tag")
except Exception as e:
    print(f"❌ FAILURE: Could not remove parent after children gone: {e}")
    sys.exit(1)

# 9. Verify final state
cmd = ["sudo", "sqlite3", test.db_path, "SELECT name FROM tags WHERE name IN ('parent', 'child1', 'child2', 'empty', 'withfile')"]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
lines = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
if len(lines) == 1 and lines[0] == 'withfile':
    print("✅ Correct tags remain (only 'withfile')")
else:
    print(f"❌ FAILURE: Unexpected remaining tags: {lines}")
    sys.exit(1)

print("✅ RMDIR TEST PASSED")