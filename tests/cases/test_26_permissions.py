# FILE: tests/cases/test_26_permissions.py
# Phase 26: Unlocking Taxonomy (UX Fix)
# Test that /magic/tags directory has write permissions (0o755)

from common import MagicTest
import os
import sys
import subprocess

test = MagicTest()
print("--- TEST 26: Permissions (UX Fix) ---")

tags_dir = os.path.join(test.mount_point, "tags")

# Test 1: Verify os.access(..., os.W_OK) is True for tags directory
print("[Test 1] Checking write access permission...")
try:
    if os.access(tags_dir, os.W_OK):
        print("✅ PASS: Tags directory has write permission")
    else:
        print("❌ FAIL: Tags directory is not writable")
        sys.exit(1)
except Exception as e:
    print(f"❌ FAIL: Error checking access: {e}")
    sys.exit(1)

# Test 2: Attempt os.mkdir(...) inside the tags directory
print("[Test 2] Creating new directory inside tags...")
test_dir_name = "test_new_tag_26"
test_dir_path = os.path.join(tags_dir, test_dir_name)

# Clean up first if it exists (just in case)
try:
    if os.path.exists(test_dir_path):
        os.rmdir(test_dir_path)
        print(f"[Setup] Cleaned up existing test directory")
except:
    pass

try:
    os.mkdir(test_dir_path)
    print("✅ PASS: Successfully created directory inside tags")
except Exception as e:
    print(f"❌ FAIL: Could not create directory inside tags: {e}")
    sys.exit(1)

# Test 3: Assert the directory is created and accessible
print("[Test 3] Verifying created directory...")
if os.path.exists(test_dir_path) and os.path.isdir(test_dir_path):
    print("✅ PASS: Directory confirmed created and accessible")
else:
    print("❌ FAIL: Directory not found or not accessible")
    sys.exit(1)

# Test 4: Check permissions of the parent tags directory via stat
print("[Test 4] Checking actual permissions via stat...")
try:
    stat_info = os.stat(tags_dir)
    mode = stat_info.st_mode
    # Extract permission bits (last 9 bits)
    perm = mode & 0o777
    expected_perm = 0o755

    if perm == expected_perm:
        print(f"✅ PASS: Tags directory has correct permissions: {oct(perm)}")
    else:
        print(f"❌ FAIL: Tags directory has wrong permissions: {oct(perm)}, expected {oct(expected_perm)}")
        sys.exit(1)
except Exception as e:
    print(f"❌ FAIL: Error checking stat: {e}")
    sys.exit(1)

# Test 5: Verify the new directory is also writable (for cleanup)
print("[Test 5] Verifying new directory is writable...")
if os.access(test_dir_path, os.W_OK):
    print("✅ PASS: New directory is writable")
else:
    print("❌ FAIL: New directory is not writable")
    sys.exit(1)

# Cleanup
print("[Cleanup] Removing test directory...")
try:
    os.rmdir(test_dir_path)
    print("✅ Cleanup complete")
except Exception as e:
    print(f"[WARN] Cleanup failed: {e}")

print("✅ ALL TESTS PASSED - Phase 26 Permissions Test Successful!")