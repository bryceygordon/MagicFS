#!/usr/bin/env python3
"""
TEST 39: Phase 39 - The Polite Inbox (Writable Access) - Atomic Unit

This test reproduces the exact requirements for Phase 39:
1. os.access("/tmp/magicfs-test-mount/inbox", os.W_OK) -> Expect True
2. Write file to /inbox via open(..., 'w')
3. os.rename("/inbox/file.txt", "/tags/finance/file.txt")

This test MUST FAIL on the current implementation.
"""

import os
import sys
import time

def main():
    print("=== TEST 39: Phase 39 - The Polite Inbox (Writable Access) ===")
    print("Atomic Unit Test: Inbox Write + Rename Operations")
    print()

    # Configuration from arguments
    if len(sys.argv) < 4:
        print("Usage: test_39_inbox_write.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    print(f"[Setup] Mount Point: {mount_point}")
    print(f"[Setup] Database: {db_path}")
    print(f"[Setup] Watch Directory: {watch_dir}")
    print()

    inbox_dir = os.path.join(mount_point, "inbox")
    tags_dir = os.path.join(mount_point, "tags")
    finance_dir = os.path.join(tags_dir, "finance")

    # Step 1: Check write permission
    print("--- Step 1: os.access(inbox, os.W_OK) ---")
    print(f"Path: {inbox_dir}")

    try:
        has_write = os.access(inbox_dir, os.W_OK)
        print(f"Result: {has_write}")

        if not has_write:
            print("❌ EXPECTED FAILURE: Inbox does not have write permission")
            print("   This is the bug we're testing for")
            print("   Expected: True, Got: False")
            sys.exit(1)
        else:
            print("✅ Permission check passed (bug may be fixed)")
    except Exception as e:
        print(f"❌ ERROR: Failed to check permissions: {e}")
        sys.exit(1)

    # Step 2: Write file to inbox
    print("\n--- Step 2: Write file to inbox ---")
    test_file = "file.txt"
    test_content = "Test content for Phase 39"
    inbox_file = os.path.join(inbox_dir, test_file)

    print(f"Target: {inbox_file}")
    print(f"Content: {test_content}")

    try:
        with open(inbox_file, 'w') as f:
            f.write(test_content)
        print("✅ File write operation completed")
    except Exception as e:
        print(f"❌ EXPECTED FAILURE: Cannot write to inbox: {e}")
        print("   This reveals the write permission bug")
        sys.exit(1)

    # Step 3: Rename/move file from inbox to tag
    print("\n--- Step 3: os.rename(inbox_file, tag_file) ---")
    finance_file = os.path.join(finance_dir, test_file)

    print(f"Source: {inbox_file}")
    print(f"Target: {finance_file}")

    # Ensure target directory exists
    try:
        os.makedirs(finance_dir, exist_ok=True)
    except Exception as e:
        print(f"⚠️  Warning: Could not create finance directory: {e}")

    try:
        os.rename(inbox_file, finance_file)
        print("✅ Rename operation completed")
    except Exception as e:
        print(f"❌ EXPECTED FAILURE: Cannot rename/move from inbox: {e}")
        print("   This reveals the rename permission bug")

        # Check if source still exists
        if os.path.exists(inbox_file):
            print(f"   Source still exists: {inbox_file}")
        if os.path.exists(finance_file):
            print(f"   Target exists: {finance_file}")

        sys.exit(1)

    # If we get here, all operations worked (bug is fixed)
    print("\n--- RESULTS ---")
    print("✅ All operations succeeded")
    print("✅ Phase 39 implementation is complete")

    # Cleanup
    try:
        if os.path.exists(finance_file):
            os.remove(finance_file)
    except:
        pass

if __name__ == "__main__":
    main()