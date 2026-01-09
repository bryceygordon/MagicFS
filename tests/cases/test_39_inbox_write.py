#!/usr/bin/env python3
"""
Test 39: Inbox Write Operations (Phase 17 - Universal Ingestion)
Verifies that /inbox acts as a functional landing zone for file ingestion.
"""

from common import MagicTest
import os
import subprocess
import time
import sys

test = MagicTest()
print("--- TEST 39: Inbox Write Operations (Phase 17) ---")

# Wait for daemon to stabilize and permissions to be applied
time.sleep(2)

# Test 1: Verify inbox tag exists in database
print("\n[Test 1] Checking database for inbox tag...")
try:
    # Use sudo for sqlite3 to avoid permission issues
    # This is the pattern used in other tests (test_10_refresh.py)
    cmd = ["sudo", "sqlite3", test.db_path, "SELECT tag_id, name FROM tags WHERE name = 'inbox'"]
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)

    if result.stdout.strip():
        output = result.stdout.strip()
        # Output is like: "1|inbox"
        parts = output.split("|")
        if len(parts) >= 2:
            tag_id = parts[0]
            name = parts[1]
            print(f"✅ Found inbox tag: tag_id={tag_id}, name='{name}'")
        else:
            print(f"✅ Found inbox tag: {output}")
    else:
        print("❌ Inbox tag missing from database")
        # Debug: show all tags
        debug_cmd = ["sudo", "sqlite3", test.db_path, "SELECT tag_id, name FROM tags;"]
        debug_result = subprocess.run(debug_cmd, capture_output=True, text=True)
        print(f"   Current tags:\n{debug_result.stdout}")
        sys.exit(1)
except subprocess.CalledProcessError as e:
    print(f"❌ Database query failed: {e}")
    print(f"   stdout: {e.stdout}")
    print(f"   stderr: {e.stderr}")
    sys.exit(1)
except Exception as e:
    print(f"❌ Unexpected error: {e}")
    sys.exit(1)

# Test 2: Write to inbox
print("\n[Test 2] Writing file to /inbox/...")
inbox_path = os.path.join(test.mount_point, "inbox", "test_doc.txt")
test_content = "This is a test document for Phase 17 inbox ingestion."

try:
    with open(inbox_path, "w") as f:
        f.write(test_content)
    print(f"✅ Write succeeded: {inbox_path}")
except Exception as e:
    print(f"❌ Write failed: {e}")
    test.dump_logs()
    sys.exit(1)

# Test 3: Verify physical persistence
print("\n[Test 3] Verifying physical persistence...")
physical_path = os.path.join(test.watch_dir, "_imported", "test_doc.txt")

if os.path.exists(physical_path):
    with open(physical_path, "r") as f:
        saved_content = f.read()
    if saved_content == test_content:
        print(f"✅ Physical file exists with correct content: {physical_path}")
    else:
        print(f"❌ Content mismatch")
        sys.exit(1)
else:
    print(f"❌ Physical file missing: {physical_path}")
    sys.exit(1)

# Test 4: Verify database linkage
print("\n[Test 4] Verifying database linkage...")
try:
    # Get file_id using sudo
    cmd1 = ["sudo", "sqlite3", test.db_path, "SELECT file_id FROM file_registry WHERE abs_path LIKE '%test_doc.txt'"]
    result1 = subprocess.run(cmd1, capture_output=True, text=True, check=True)

    if not result1.stdout.strip():
        print("❌ File not in registry")
        sys.exit(1)

    file_id = result1.stdout.strip()
    print(f"✅ File in registry with ID: {file_id}")

    # Check file_tags link
    cmd2 = ["sudo", "sqlite3", test.db_path, f"""
        SELECT t.name, ft.display_name
        FROM file_tags ft
        JOIN tags t ON ft.tag_id = t.tag_id
        WHERE ft.file_id = {file_id}
    """]
    result2 = subprocess.run(cmd2, capture_output=True, text=True, check=True)

    tag_output = result2.stdout.strip()
    if tag_output:
        parts = tag_output.split("|")
        if len(parts) >= 2 and parts[0] == 'inbox':
            print(f"✅ Linked to inbox tag as '{parts[1]}'")
        else:
            print(f"❌ Not linked to inbox tag: {tag_output}")
            sys.exit(1)
    else:
        print("❌ No tag linkage found")
        sys.exit(1)

except subprocess.CalledProcessError as e:
    print(f"❌ Database verification failed: {e}")
    print(f"   stdout: {e.stdout}")
    print(f"   stderr: {e.stderr}")
    sys.exit(1)

# Test 5: Read back from inbox
print("\n[Test 5] Reading back from inbox...")
try:
    with open(inbox_path, "r") as f:
        read_content = f.read()

    if read_content == test_content:
        print("✅ Read-back successful")
    else:
        print(f"❌ Read-back content mismatch")
        print(f"   Expected: {test_content}")
        print(f"   Got: {read_content}")
        sys.exit(1)
except Exception as e:
    print(f"❌ Read-back failed: {e}")
    sys.exit(1)

print("\n" + "="*60)
print("🎉 ALL TESTS PASSED")
print("✅ Inbox is functional as a landing zone")
print("✅ Physical persistence to _imported/ works")
print("✅ Database linkage to tag_id=1 works")
print("✅ Read/write operations work")
print("="*60)