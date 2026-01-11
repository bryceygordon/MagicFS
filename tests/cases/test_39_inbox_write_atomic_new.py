#!/usr/bin/env python3
"""
TEST 39: Phase 39 - The Polite Inbox (Writable Access)

This test verifies the Inbox writable access requirements:
1. os.access("/mount/inbox", os.W_OK) -> Should return True (write permission)
2. Create file in /inbox via open(..., 'w')
3. os.rename("/inbox/file.txt", "/tags/finance/file.txt") -> Move file from inbox to tag

This test will FAIL on the current implementation, revealing the bug.
"""

import os
import sys
import time
import subprocess

def main():
    print("=== TEST 39: Phase 39 - The Polite Inbox (Writable Access) ===")

    # Configuration from arguments
    if len(sys.argv) < 4:
        print("Usage: test_39_inbox_write_atomic_new.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    print(f"[Setup] Mount Point: {mount_point}")
    print(f"[Setup] Database: {db_path}")
    print(f"[Setup] Watch Directory: {watch_dir}")

    # Test paths
    inbox_dir = os.path.join(mount_point, "inbox")
    tags_dir = os.path.join(mount_point, "tags")
    finance_dir = os.path.join(tags_dir, "finance")

    # Test filename
    test_filename = "test_polite_inbox.txt"
    test_content = "This is a test file for polite inbox operations."

    inbox_file_path = os.path.join(inbox_dir, test_filename)
    finance_file_path = os.path.join(finance_dir, test_filename)

    print(f"[Target] Inbox file: {inbox_file_path}")
    print(f"[Target] Finance file: {finance_file_path}")

    # Clean up any existing test files
    try:
        if os.path.exists(inbox_file_path):
            os.remove(inbox_file_path)
    except:
        pass

    try:
        if os.path.exists(finance_file_path):
            os.remove(finance_file_path)
    except:
        pass

    time.sleep(0.5)

    # STEP 1: Test write permission on inbox
    print("\n--- Test 1: Check Write Permission ---")
    print(f"Checking os.access('{inbox_dir}', os.W_OK)...")

    has_write_access = os.access(inbox_dir, os.W_OK)
    print(f"Result: {has_write_access}")

    if not has_write_access:
        print("❌ FAILURE: Inbox directory does not have write permission")
        print("   This reveals the permission bug (0o555 instead of 0o755)")
        sys.exit(1)  # Expected failure

    print("✅ SUCCESS: Inbox has write permission (this means bug is fixed)")

    # STEP 2: Create file in inbox
    print("\n--- Test 2: Create File in Inbox ---")
    print(f"Opening {inbox_file_path} for writing...")

    try:
        with open(inbox_file_path, "w") as f:
            f.write(test_content)
        print("✅ SUCCESS: File created in inbox")
    except Exception as e:
        print(f"❌ FAILURE: Cannot create file in inbox: {e}")
        print("   This reveals the write access bug")
        sys.exit(1)  # Expected failure

    # Verify the file exists
    time.sleep(0.5)
    if not os.path.exists(inbox_file_path):
        print("❌ FAILURE: File was not actually created")
        sys.exit(1)

    print(f"✅ File exists: {inbox_file_path}")

    # STEP 3: Move file from inbox to tag directory
    print("\n--- Test 3: Move File from Inbox to Tag ---")
    print(f"Attempting os.rename('{inbox_file_path}', '{finance_file_path}')...")

    # First ensure the finance tag directory exists
    if not os.path.exists(finance_dir):
        print(f"   Creating target directory: {finance_dir}")
        try:
            os.makedirs(finance_dir, exist_ok=True)
        except Exception as e:
            print(f"❌ FAILURE: Cannot create target directory: {e}")
            sys.exit(1)

    try:
        os.rename(inbox_file_path, finance_file_path)
        print("✅ SUCCESS: File moved from inbox to finance tag")
    except Exception as e:
        print(f"❌ FAILURE: Cannot move file from inbox: {e}")
        print("   This reveals the rename/permission bug")

        # Verify what happened to the original file
        if os.path.exists(inbox_file_path):
            print("   Original file still exists in inbox")
        if os.path.exists(finance_file_path):
            print("   Target file exists (but move failed?)")

        sys.exit(1)  # Expected failure

    # Verify the move actually worked
    time.sleep(0.5)
    if not os.path.exists(finance_file_path):
        print("❌ FAILURE: Target file does not exist after rename")
        sys.exit(1)

    if os.path.exists(inbox_file_path):
        print("❌ FAILURE: Original file still exists in inbox")
        sys.exit(1)

    # Verify content
    with open(finance_file_path, "r") as f:
        content = f.read()

    if content != test_content:
        print(f"❌ FAILURE: Content mismatch")
        print(f"   Expected: {test_content}")
        print(f"   Got: {content}")
        sys.exit(1)

    print("✅ File content verified")

    # STEP 4: Verify database state (file should be tagged with Tag ID 1, then moved to finance tag)
    print("\n--- Test 4: Verify Database State ---")

    if not os.path.exists(db_path):
        print(f"❌ FAILURE: Database not found at {db_path}")
        sys.exit(1)

    # The finance tag might have a different tag_id. Let's check what tags exist and verify the file is indexed
    try:
        # Get the real path that should be in the database
        real_finance_path = os.path.join(watch_dir, "finance", test_filename)  # This assumes tag directories are created in watch dir

        # Let's check if file shows up in file_registry
        result = subprocess.run([
            "sudo", "sqlite3", db_path,
            "SELECT abs_path FROM file_registry WHERE abs_path LIKE '%test_polite_inbox.txt'"
        ], capture_output=True, text=True, check=True)

        found_files = result.stdout.strip().split('\n')
        found_files = [f for f in found_files if f]  # Remove empty strings

        if not found_files:
            print("⚠️  Warning: File not found in database registry yet")
            print("   This might be normal if the daemon hasn't processed it")
        else:
            print(f"✅ Found file in registry: {found_files}")

            # Check tags for this file
            for file_path in found_files:
                tag_query = f"""
                    SELECT t.name, ft.display_name
                    FROM file_tags ft
                    JOIN tags t ON ft.tag_id = t.tag_id
                    WHERE ft.file_id = (SELECT file_id FROM file_registry WHERE abs_path = '{file_path}')
                """
                result = subprocess.run([
                    "sudo", "sqlite3", db_path, tag_query
                ], capture_output=True, text=True, check=True)

                if result.stdout.strip():
                    print(f"   Tags: {result.stdout.strip()}")
                else:
                    print(f"   No tags found for file")

    except subprocess.CalledProcessError as e:
        print(f"⚠️  Database query failed: {e}")
        print("   This might indicate database locking or other issues")

    # Final cleanup
    print("\n--- Cleanup ---")
    try:
        if os.path.exists(finance_file_path):
            os.remove(finance_file_path)
            print("✅ Cleaned up finance file")
    except:
        pass

    print("\n" + "="*70)
    print("✅ TEST 39 COMPLETED SUCCESSFULLY")
    print("="*70)
    print("All operations work:")
    print("  1. Write permission check: ✅")
    print("  2. File creation in inbox: ✅")
    print("  3. Move from inbox to tag: ✅")
    print("\nThis means Phase 39 implementation is complete!")

if __name__ == "__main__":
    main()