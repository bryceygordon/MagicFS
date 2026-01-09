#!/usr/bin/env python3
"""
Test 34: Query Performance - Index Verification
Phase 16: Query Performance for Sidecar GUI

This test verifies that the database schema has the required indices
for optimal query performance in the Sidecar application.
"""

import sqlite3
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

    try:
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()

        # Required indices according to Phase 16 spec
        required_indices = {
            "tags": ["idx_tags_parent"],
            "file_tags": ["idx_file_tags_tag"]
        }

        all_passed = True

        for table, expected_indices in required_indices.items():
            print(f"\nChecking table: {table}")

            # Query for existing indices
            cursor.execute(f"PRAGMA index_list('{table}')")
            existing_indices = [row[1] for row in cursor.fetchall()]

            print(f"  Existing indices: {existing_indices}")

            for expected_idx in expected_indices:
                if expected_idx in existing_indices:
                    print(f"  ✅ Found required index: {expected_idx}")
                else:
                    print(f"  ❌ MISSING required index: {expected_idx}")
                    all_passed = False

        conn.close()
        return all_passed

    except sqlite3.Error as e:
        print(f"❌ FAILURE: Database error: {e}")
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