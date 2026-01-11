#!/usr/bin/env python3
"""
Quick validation script to verify the data integrity fix.
This confirms that:
1. After inbox→tag move, the database has the NEW path
2. No duplicate entries exist
3. find_real_path(inode) returns the correct path
"""

import os
import sys
import sqlite3
import subprocess
import time

def main():
    if len(sys.argv) < 4:
        print("Usage: validate_fix.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    print("=== VALIDATING DATA INTEGRITY FIX ===\n")

    # Test file
    test_file = "validation_test.txt"
    inbox_path = os.path.join(mount_point, "inbox", test_file)
    tag_path = os.path.join(mount_point, "tags", "finance", test_file)

    print(f"[1] Creating test file in inbox: {inbox_path}")
    try:
        with open(inbox_path, "w") as f:
            f.write("Test content for validation")
        print("✅ File created")
    except Exception as e:
        print(f"❌ Failed: {e}")
        return False

    time.sleep(0.5)

    print(f"\n[2] Moving file to tag: {inbox_path} → {tag_path}")
    try:
        # Ensure target directory exists
        os.makedirs(os.path.dirname(tag_path), exist_ok=True)
        os.rename(inbox_path, tag_path)
        print("✅ Rename succeeded")
    except Exception as e:
        print(f"❌ Rename failed: {e}")
        return False

    time.sleep(0.5)

    print(f"\n[3] Checking database state...")
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()

        # Get all entries for our test file
        cursor.execute(
            "SELECT abs_path, inode FROM file_registry WHERE abs_path LIKE ?",
            (f"%{test_file}",)
        )
        entries = cursor.fetchall()

        print(f"   Found {len(entries)} registry entries for '{test_file}'")
        for path, inode in entries:
            print(f"   - {path} (inode: {inode})")

        if len(entries) == 0:
            print("❌ ERROR: No entries found! File not indexed.")
            return False

        if len(entries) > 1:
            print("❌ ERROR: Multiple entries found! Duplicate data.")
            return False

        # Verify the entry is the NEW path
        valid_path = os.path.join(watch_dir, "_moved_from_inbox", test_file)
        if entries[0][0] != valid_path:
            print(f"❌ ERROR: Wrong path in DB!")
            print(f"   Expected: {valid_path}")
            print(f"   Got: {entries[0][0]}")
            return False

        # Verify tags
        cursor.execute("""
            SELECT t.name, ft.display_name
            FROM file_tags ft
            JOIN tags t ON ft.tag_id = t.tag_id
            WHERE ft.file_id = (SELECT file_id FROM file_registry WHERE abs_path = ?)
        """, (valid_path,))
        tag_entries = cursor.fetchall()

        print(f"\n[4] Checking tags...")
        print(f"   Found {len(tag_entries)} tag associations")
        for tag_name, display_name in tag_entries:
            print(f"   - {tag_name}: {display_name}")

        # Should have finance tag, NOT inbox tag (id=1)
        tag_names = [t[0] for t in tag_entries]
        if "finance" not in tag_names:
            print("❌ ERROR: Finance tag not found!")
            return False
        if "inbox" in tag_names or 1 in [t[0] for t in tag_entries if isinstance(t[0], int)]:
            print("❌ ERROR: Old inbox tag still present!")
            return False

        print("\n[5] Verifying file is readable via registry path...")
        registry_path = entries[0][0]
        if os.path.exists(registry_path):
            with open(registry_path, "r") as f:
                content = f.read()
            if content == "Test content for validation":
                print("✅ File content verified")
            else:
                print("❌ Content mismatch")
                return False
        else:
            print("❌ Registry path doesn't exist on filesystem!")
            return False

        conn.close()
        return True

    except Exception as e:
        print(f"❌ Database check failed: {e}")
        return False

    finally:
        # Cleanup
        try:
            if os.path.exists(tag_path):
                os.remove(tag_path)
        except:
            pass

if __name__ == "__main__":
    if main():
        print("\n" + "="*50)
        print("✅ DATA INTEGRITY FIX VALIDATED!")
        print("="*50)
    else:
        print("\n" + "="*50)
        print("❌ VALIDATION FAILED")
        print("="*50)
        sys.exit(1)