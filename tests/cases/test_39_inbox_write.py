#!/usr/bin/env python3
"""
Test 39: Phase 17 - Universal Ingestion (The Magic Inbox)

This test verifies the new system-managed inbox implementation:
- Files written to /magic/inbox go to SYSTEM domain, not user watch directories
- System domain is $XDG_DATA_HOME/magicfs/inbox (default)
- Can be overridden with MAGICFS_DATA_DIR environment variable
- Tag ID 1 (Inbox) is automatically applied
"""

import os
import sqlite3
import subprocess
import time
import sys
import shutil

def main():
    print("=== TEST 39: Phase 17 - Universal Ingestion (The Magic Inbox) ===")

    # Configuration from arguments
    if len(sys.argv) < 4:
        print("Usage: test_39_inbox_write.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    # Get system inbox directory from environment (set by test runner)
    system_data_dir = os.environ.get("MAGICFS_DATA_DIR")
    if not system_data_dir:
        print("❌ FAILURE: MAGICFS_DATA_DIR environment variable not set")
        sys.exit(1)

    system_inbox_dir = os.path.join(system_data_dir, "inbox")

    print(f"[Setup] System Data Dir: {system_data_dir}")
    print(f"[Setup] System Inbox Dir: {system_inbox_dir}")
    print(f"[Setup] Mount Point: {mount_point}")
    print(f"[Setup] Watch Dir (user): {watch_dir}")

    # Verify system inbox exists and is separate from watch_dir
    if system_inbox_dir.startswith(watch_dir):
        print("❌ FAILURE: System inbox cannot be inside watch directory")
        sys.exit(1)

    if not os.path.exists(system_inbox_dir):
        print(f"❌ FAILURE: System inbox directory does not exist: {system_inbox_dir}")
        sys.exit(1)

    # Clean any existing test files in system inbox (use sudo for 0o700 permissions)
    try:
        subprocess.run(["sudo", "sh", "-c", f"rm -f {system_inbox_dir}/*"], check=False)
    except:
        pass

    # Clean any existing test files in watch directory
    user_import_dir = os.path.join(watch_dir, "_imported")
    if os.path.exists(user_import_dir):
        for f in os.listdir(user_import_dir):
            try:
                os.remove(os.path.join(user_import_dir, f))
            except:
                pass

    # Give system a moment to settle
    time.sleep(1.0)

    # Test 1: Write to virtual inbox via FUSE
    print("\n--- Test 1: Write to Virtual Inbox (/magic/inbox/test_file.txt) ---")
    virtual_inbox_path = os.path.join(mount_point, "inbox", "test_file.txt")
    test_content = "This is a test file for the inbox"

    try:
        # Ensure inbox exists in FUSE view
        if not os.path.exists(os.path.join(mount_point, "inbox")):
            print("❌ FAILURE: /magic/inbox directory does not exist")
            sys.exit(1)

        # Write to virtual inbox
        with open(virtual_inbox_path, "w") as f:
            f.write(test_content)

        print("✓ Write to virtual inbox succeeded")

    except Exception as e:
        print(f"❌ FAILURE: Could not write to virtual inbox: {e}")
        sys.exit(1)

    # Wait for processing
    time.sleep(2.0)

    # Assert 1: File should exist in SYSTEM inbox, not user watch directory
    system_file_path = os.path.join(system_inbox_dir, "test_file.txt")
    user_imported_file = os.path.join(watch_dir, "_imported", "test_file.txt")

    print(f"\n--- Verification 1: Physical Location ---")
    print(f"Checking system inbox: {system_file_path}")
    print(f"Checking user import dir: {user_imported_file}")

    # Check if file exists using sudo (due to 700 permissions on system inbox)
    try:
        exists_check = subprocess.run(
            ["sudo", "test", "-f", system_file_path],
            capture_output=True,
            check=True
        )
        file_exists = True
    except subprocess.CalledProcessError:
        file_exists = False

    if file_exists:
        print("✓ File correctly created in system inbox")
        # Read content using sudo
        try:
            content_check = subprocess.run(
                ["sudo", "cat", system_file_path],
                capture_output=True,
                text=True,
                check=True
            )
            content = content_check.stdout.strip()
            if content == test_content:
                print("✓ Content matches")
            else:
                print(f"❌ FAILURE: Content mismatch. Expected: '{test_content}', Got: '{content}'")
                sys.exit(1)
        except subprocess.CalledProcessError as e:
            print(f"❌ FAILURE: Could not read file content: {e}")
            sys.exit(1)
    else:
        print("❌ FAILURE: File not found in system inbox")
        sys.exit(1)

    if os.path.exists(user_imported_file):
        print("❌ FAILURE: File incorrectly created in user watch directory")
        sys.exit(1)
    else:
        print("✓ File correctly NOT in user watch directory")

    # Assert 2: Database verification - file in registry and tagged as Inbox
    print(f"\n--- Verification 2: Database State ---")

    if not os.path.exists(db_path):
        print(f"❌ FAILURE: Database not found at {db_path}")
        sys.exit(1)

    try:
        # Use sudo sqlite3 to avoid WAL permission issues
        cmd = ["sudo", "sqlite3", db_path, f"SELECT abs_path FROM file_registry WHERE abs_path = '{system_file_path}'"]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)

        if result.stdout.strip() == system_file_path:
            print("✓ File registered in database")
        else:
            print(f"❌ FAILURE: File not in registry. Result: {result.stdout.strip()}")
            sys.exit(1)

        # Check file_tags for Tag ID 1 (Inbox)
        cmd = ["sudo", "sqlite3", db_path, f"""
            SELECT t.name, ft.display_name
            FROM file_tags ft
            JOIN tags t ON ft.tag_id = t.tag_id
            WHERE ft.file_id = (SELECT file_id FROM file_registry WHERE abs_path = '{system_file_path}')
        """]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)

        if "inbox" in result.stdout.lower():
            print(f"✓ File linked to inbox tag: {result.stdout.strip()}")
        else:
            print(f"❌ FAILURE: File not linked to inbox tag. Result: {result.stdout.strip()}")
            sys.exit(1)

    except subprocess.CalledProcessError as e:
        print(f"❌ FAILURE: Database query failed: {e}")
        print(f"stdout: {e.stdout}")
        print(f"stderr: {e.stderr}")
        sys.exit(1)

    # Test 2: Physical file drop into system inbox
    print(f"\n--- Test 2: Physical Drop into System Inbox ---")
    physical_drop_file = os.path.join(system_inbox_dir, "dropped_file.txt")
    drop_content = "Dropped directly into system inbox"

    # Use sudo echo to write to protected directory
    try:
        subprocess.run(
            ["sudo", "sh", "-c", f"echo '{drop_content}' > '{physical_drop_file}'"],
            check=True,
            capture_output=True
        )
        print(f"✓ Created physical file: {physical_drop_file}")
    except subprocess.CalledProcessError as e:
        print(f"❌ FAILURE: Could not create physical drop file: {e}")
        sys.exit(1)

    # Wait for Librarian to pick it up
    time.sleep(2.0)

    # Verify it appears in database
    try:
        cmd = ["sudo", "sqlite3", db_path, f"SELECT abs_path FROM file_registry WHERE abs_path = '{physical_drop_file}'"]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)

        if result.stdout.strip() == physical_drop_file:
            print("✓ Physical drop file indexed in database")
        else:
            print(f"❌ FAILURE: Physical drop file not indexed. Result: {result.stdout.strip()}")
            sys.exit(1)

        # Verify it's tagged as inbox
        cmd = ["sudo", "sqlite3", db_path, f"""
            SELECT t.name FROM file_tags ft
            JOIN tags t ON ft.tag_id = t.tag_id
            WHERE ft.file_id = (SELECT file_id FROM file_registry WHERE abs_path = '{physical_drop_file}')
        """]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)

        if "inbox" in result.stdout.lower():
            print("✓ Physical drop file automatically tagged as inbox")
        else:
            print(f"❌ FAILURE: Physical drop file not automatically tagged. Result: {result.stdout.strip()}")
            sys.exit(1)

    except subprocess.CalledProcessError as e:
        print(f"❌ FAILURE: Database verification failed: {e}")
        sys.exit(1)

    # Test 3: Verify inbox directory exists with correct permissions
    print(f"\n--- Verification 3: System Inbox Properties ---")

    inbox_stat = os.stat(system_inbox_dir)
    inbox_mode = inbox_stat.st_mode & 0o777

    if inbox_mode == 0o700 or inbox_mode == 0o755:
        print(f"✓ System inbox has correct permissions: {oct(inbox_mode)}")
    else:
        print(f"⚠️  Warning: Unexpected permissions {oct(inbox_mode)} (expected 0o700 or 0o755)")

    # Test 4: Verify old _imported directory is NOT used
    print(f"\n--- Verification 4: Legacy Import Directory ---")

    # Check if watch_dir has _imported subdirectory
    legacy_import_dir = os.path.join(watch_dir, "_imported")

    if os.path.exists(legacy_import_dir):
        # Should be empty or contain old files only
        contents = os.listdir(legacy_import_dir)
        if any("test_file" in f or "dropped_file" in f for f in contents):
            print(f"❌ FAILURE: Legacy import directory contains new files: {contents}")
            sys.exit(1)
        else:
            print(f"✓ Legacy import dir exists but doesn't contain new files")
    else:
        print("✓ Legacy import directory doesn't exist (clean separation)")

    # Final cleanup
    print(f"\n--- Cleanup ---")
    try:
        # Use sudo for cleanup due to restrictive permissions
        subprocess.run(["sudo", "rm", "-f", system_file_path], check=False)
        subprocess.run(["sudo", "rm", "-f", physical_drop_file], check=False)
        print("✓ Cleanup completed")
    except Exception as e:
        print(f"⚠️  Cleanup warning: {e}")

    print("\n" + "="*60)
    print("✅ TEST 39 PASSED: Phase 17 Universal Ingestion")
    print("="*60)
    print("✓ Virtual inbox write works")
    print("✓ Files go to system domain, not user watch directory")
    print("✓ Automatic tagging with Tag ID 1 (Inbox)")
    print("✓ Physical drop detection working")
    print("✓ Legacy import directory not used")

if __name__ == "__main__":
    main()