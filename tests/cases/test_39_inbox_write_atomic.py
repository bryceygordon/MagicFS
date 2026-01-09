#!/usr/bin/env python3
"""
TEST 39: Phase 24 - Inbox Write Compliance (Atomic Write Test)

This test reproduces the atomic save issue:
1. CREATE works (test_file.part is routed to system inbox)
2. RENAME fails (test_file.part -> test_file.txt fails due to HollowDrive::rename restrictions)

This confirms the bug before implementing the fix.
"""

import os
import sys
import time

# Import the MagicTest class
from common import MagicTest

def main():
    print("--- TEST 39: Inbox Atomic Write (Create + Rename) ---")

    # Initialize test with standard arguments
    test = MagicTest()

    # Get system inbox directory from environment (set by test runner)
    system_data_dir = os.environ.get("MAGICFS_DATA_DIR")
    if not system_data_dir:
        print("❌ FAILURE: MAGICFS_DATA_DIR environment variable not set")
        sys.exit(1)

    system_inbox_dir = os.path.join(system_data_dir, "inbox")

    print(f"[Setup] Mount Point: {test.mount_point}")
    print(f"[Setup] System Inbox: {system_inbox_dir}")

    # 1. Define paths
    inbox_dir = os.path.join(test.mount_point, "inbox")
    temp_filename = "atomic_save.part"
    final_filename = "atomic_save.txt"
    temp_path = os.path.join(inbox_dir, temp_filename)
    final_path = os.path.join(inbox_dir, final_filename)

    # 2. Verify Inbox Exists in FUSE view
    if not os.path.exists(inbox_dir):
        print("❌ FAILURE: Inbox directory not found in mount point.")
        test.dump_logs()
        sys.exit(1)

    print("✅ Inbox directory exists in FUSE view")

    # 3. Test CREATE (Should Work - this is the "create" phase of atomic save)
    print(f"\n--- Test 1: CREATE (Should Work) ---")
    print(f"[Action] Creating temporary file: {temp_filename}")

    try:
        with open(temp_path, "w") as f:
            f.write("Content pending atomic save.")
        print("✅ CREATE successful")

        # Verify the file appears in system inbox
        time.sleep(1.0)  # Allow indexing
        system_temp_path = os.path.join(system_inbox_dir, temp_filename)

        # Check if file exists in system inbox using sudo (due to 700 permissions)
        import subprocess
        try:
            exists_check = subprocess.run(
                ["sudo", "test", "-f", system_temp_path],
                capture_output=True,
                check=True
            )
            print(f"✅ File routed to system inbox: {system_temp_path}")
        except subprocess.CalledProcessError:
            print(f"❌ FAILURE: File not found in system inbox: {system_temp_path}")
            test.dump_logs()
            sys.exit(1)

    except Exception as e:
        print(f"❌ FAILURE: Create failed: {e}")
        test.dump_logs()
        sys.exit(1)

    # 4. Test RENAME (Expected to Fail with current implementation)
    print(f"\n--- Test 2: RENAME (Expected to Fail) ---")
    print(f"[Action] Renaming {temp_filename} -> {final_filename}")
    print(f"        (This simulates atomic save: .part -> final)")

    try:
        os.rename(temp_path, final_path)
        print("✅ RENAME successful (This means the fix is already active!)")
        # If rename works, we should verify the final file exists
        time.sleep(1.0)
        if os.path.exists(final_path):
            print("✅ Final file exists and is readable")
            # Clean up
            try:
                os.remove(final_path)
                print("✅ Cleanup successful")
            except:
                pass
        else:
            print("⚠️  Rename succeeded but final file not found")

    except OSError as e:
        print(f"❌ EXPECTED FAILURE: Rename failed: {e}")
        print(f"   Error Code: {e.errno}")
        print(f"   Error Type: {type(e).__name__}")

        # This is the expected behavior before the fix
        print("\n--- DIAGNOSIS ---")
        print("This confirms the bug: HollowDrive::rename rejects INODE_INBOX operations.")
        print("The create() call routes to system inbox, but rename() fails due to")
        print("the restriction that only persistent tags (high bit set) are allowed.")

        # Clean up the temp file from system inbox
        print("\n--- Cleanup ---")
        try:
            subprocess.run(["sudo", "rm", "-f", system_temp_path], check=False)
            print("✅ Cleaned up temp file from system inbox")
        except Exception as cleanup_e:
            print(f"⚠️  Cleanup warning: {cleanup_e}")

        # Verify no final file was created
        system_final_path = os.path.join(system_inbox_dir, final_filename)
        try:
            subprocess.run(["sudo", "test", "-f", system_final_path], capture_output=True)
            print("❌ FAILURE: Final file should not exist")
            sys.exit(1)
        except subprocess.CalledProcessError:
            print("✅ Final file correctly does not exist")

        print("\n" + "="*60)
        print("✅ TEST 39 CONFIRMED: Bug reproduced successfully")
        print("="*60)
        print("Summary:")
        print("  - CREATE works: ✅")
        print("  - RENAME fails: ✅ (Expected)")
        print("  - Root cause: HollowDrive::rename restricts INODE_INBOX")
        print("\nReady for Phase 24 implementation...")
        sys.exit(0)  # Exit 0 to indicate test passed (we successfully reproduced the bug)

    except Exception as e:
        print(f"❌ UNEXPECTED ERROR: {e}")
        test.dump_logs()
        sys.exit(1)

if __name__ == "__main__":
    main()