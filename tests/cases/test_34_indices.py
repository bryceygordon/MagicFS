#!/usr/bin/env python3
"""
Test 34: Query Performance - Index Verification
Phase 16: Query Performance for Sidecar GUI

This test verifies that the database schema has the required indices
for optimal query performance in the Sidecar application.
"""

import subprocess
import sys
import os

def verify_indices():
    """
    Verify that required indices exist in the database schema.
    Returns True if all indices exist, False otherwise.
    """

    # Use standard test database path (match main.rs)
    DB_PATH = "/tmp/.magicfs_nomic/index.db"

    print("=== TEST 34: Query Performance Index Verification ===")
    print(f"Checking database: {DB_PATH}")

    if not os.path.exists(DB_PATH):
        print(f"❌ FAILURE: Database file not found: {DB_PATH}")
        print("   Make sure daemon is running and has initialized the database")
        return False

    # Required indices according to Phase 16 spec
    required_indices = {
        "tags": ["idx_tags_parent"],
        "file_tags": ["idx_file_tags_tag"]
    }

    all_passed = True

    try:
        for table, expected_indices in required_indices.items():
            print(f"\nChecking table: {table}")

            # Query for existing indices using sudo sqlite3 (avoids WAL permission issues)
            cmd = ["sudo", "sqlite3", DB_PATH, f"PRAGMA index_list('{table}')"]
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)

            # Parse output format: each line is: seq|name|unique|origin|origin|partial
            existing_indices = []
            for line in result.stdout.strip().split('\n'):
                if line:
                    parts = line.split('|')
                    if len(parts) >= 2:
                        existing_indices.append(parts[1])

            print(f"  Existing indices: {existing_indices}")

            for expected_idx in expected_indices:
                if expected_idx in existing_indices:
                    print(f"  ✅ Found required index: {expected_idx}")
                else:
                    print(f"  ❌ MISSING required index: {expected_idx}")
                    all_passed = False

        return all_passed

    except subprocess.CalledProcessError as e:
        print(f"❌ FAILURE: Database query failed: {e}")
        print(f"   stdout: {e.stdout}")
        print(f"   stderr: {e.stderr}")
        # Debug: Show database file ownership
        print("   Debug info:")
        try:
            ls_result = subprocess.run(["ls", "-la", DB_PATH], capture_output=True, text=True)
            print(f"   DB file: {ls_result.stdout.strip()}")

            ls_shm = subprocess.run(["ls", "-la", DB_PATH + "-shm"], capture_output=True, text=True)
            if ls_shm.returncode == 0:
                print(f"   DB-shm: {ls_shm.stdout.strip()}")

            ls_wal = subprocess.run(["ls", "-la", DB_PATH + "-wal"], capture_output=True, text=True)
            if ls_wal.returncode == 0:
                print(f"   DB-wal: {ls_wal.stdout.strip()}")
        except:
            pass
        return False
    except Exception as e:
        print(f"❌ FAILURE: Unexpected error: {e}")
        return False

if __name__ == "__main__":
    success = verify_indices()

    if success:
        print("\n✅ TEST 34 PASSED: All required indices exist")
        sys.exit(0)
    else:
        print("\n❌ TEST 34 FAILED: Missing required indices")
        print("\nExpected indices:")
        print("  - tags: idx_tags_parent (for fast parent lookups)")
        print("  - file_tags: idx_file_tags_tag (for fast tag lookups)")
        sys.exit(1)