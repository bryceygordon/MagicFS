# FILE: tests/cases/test_18_readonly_search.py
from common import MagicTest
import os
import time
import sys
import shutil

test = MagicTest()
print("--- TEST 18: Read-Only Search (Creation Prevention) ---")

# 1. TEST NAVIGATION (Should Succeed)
print("\n[Test 1] Verifying Navigation...")
query_dir = os.path.join(test.mount_point, "search", "valid_navigation")
try:
    os.stat(query_dir)
    print("✅ Navigation to phantom directory successful.")
except Exception as e:
    print(f"❌ FAILURE: Navigation failed: {e}")
    sys.exit(1)

# 2. TEST CREATION (Should Fail)
print("\n[Test 2] Verifying 'mkdir' Failure...")
mkdir_target = os.path.join(test.mount_point, "search", "New Folder")
try:
    os.mkdir(mkdir_target)
    print("❌ FAILURE: mkdir succeeded! Search root is not read-only.")
    # Clean up if it actually worked
    shutil.rmtree(mkdir_target)
    sys.exit(1)
except OSError as e:
    # We expect PermissionDenied (13) or Read-only file system (30)
    print(f"✅ Success: mkdir blocked with error: {e}")

# 3. TEST DEEP CREATION (Should also fail)
print("\n[Test 3] Verifying Deep 'mkdir' Failure...")
# Even inside a query, we shouldn't be able to make folders
deep_target = os.path.join(query_dir, "Deep Folder")
try:
    os.mkdir(deep_target)
    print("❌ FAILURE: Deep mkdir succeeded!")
    sys.exit(1)
except OSError as e:
    print(f"✅ Success: Deep mkdir blocked with error: {e}")

print("\n✅ READ-ONLY ARCHITECTURE VERIFIED")
