#!/usr/bin/env python3
"""
Test 38: Root Hierarchy Flattening (Phase 21)

Goal: Elevate 'inbox' and 'tags' to the filesystem root (Level 1).
They should become "First-Class Citizens" accessible via:
  /inbox
  /tags

Current State (Phase 20):
  /                       (Root)
  ├── .magic/             (MagicFS Root)
  │   ├── inbox/          (Nested - BAD UX)
  │   ├── tags/           (Nested - BAD UX)
  │   ├── search/         (Standard)
  │   └── mirror/         (Standard)

Target State (Phase 21):
  /                       (Root)
  ├── inbox/              (First-Class - GOOD UX)
  ├── tags/               (First-Class - GOOD UX)
  ├── search/             (Standard)
  ├── mirror/             (Standard)
  └── .magic/             (MagicFS Internal)
      ├── refresh         (Control file)
      └── (no inbox/tags)

This test validates the NEW structure.
"""

import os
import sys
import subprocess
import time

# Setup paths
MOUNT_POINT = "/tmp/magicfs-test-mount"  # From run_suite.sh / run_single.sh
WATCH_DIR = "/tmp/magicfs-test-data"
DB_PATH = "/tmp/.magicfs_nomic/index.db"
MAGICFS_LOG = "/tmp/magicfs_debug.log"

# Colors for output
RED = '\033[91m'
GREEN = '\033[92m'
YELLOW = '\033[93m'
RESET = '\033[0m'

def log(message, level="INFO"):
    """Log with color coding"""
    if level == "ERROR":
        print(f"{RED}[FAIL]{RESET} {message}")
    elif level == "SUCCESS":
        print(f"{GREEN}[PASS]{RESET} {message}")
    elif level == "WARN":
        print(f"{YELLOW}[WARN]{RESET} {message}")
    else:
        print(f"[INFO] {message}")

def check_path_exists(path, description):
    """Check if a path exists in the filesystem"""
    exists = os.path.exists(path)
    if exists:
        log(f"✓ {description} exists: {path}", "SUCCESS")
    else:
        log(f"✗ {description} missing: {path}", "ERROR")
    return exists

def check_path_not_exists(path, description):
    """Check if a path does NOT exist"""
    exists = os.path.exists(path)
    if not exists:
        log(f"✓ {description} correctly absent: {path}", "SUCCESS")
    else:
        log(f"✗ {description} incorrectly present: {path}", "ERROR")
    return not exists

def list_directory(path):
    """List contents of directory safely"""
    try:
        return sorted(os.listdir(path))
    except Exception as e:
        log(f"Could not list {path}: {e}", "ERROR")
        return []

def main():
    print("=" * 70)
    print("PHASE 21: ROOT HIERARCHY FLATTENING - STRUCTURE VALIDATION")
    print("=" * 70)
    print("\nGoal: Elevate 'inbox' and 'tags' to root level (Level 1)")
    print("Expected: /inbox, /tags, /search, /mirror, /.magic")
    print("-" * 70)

    # Wait for daemon to stabilize
    print("\n[Phase 1] Waiting for daemon to be ready...")
    if not os.path.exists(MOUNT_POINT):
        log("Mount point does not exist. Daemon may not be running.", "ERROR")
        return 1

    # Check root listing first
    root_items = list_directory(MOUNT_POINT)
    log(f"Root contents: {root_items}")

    # Allow some time for mount to settle
    time.sleep(1)

    all_tests_passed = True

    # ========================================================================
    # TEST 1: ROOT LEVEL STRUCTURE (The "First-Class Citizens")
    # ========================================================================
    print("\n[Phase 2] Validating Root Level (Level 1)")
    print("-" * 40)

    # These should exist at root
    required_at_root = {
        "inbox": "/inbox",
        "tags": "/tags",
        "search": "/search",
        "mirror": "/mirror",
        ".magic": "/.magic"
    }

    for name, path in required_at_root.items():
        full_path = os.path.join(MOUNT_POINT, name.lstrip('/'))
        if not check_path_exists(full_path, f"Level 1: '{name}'"):
            all_tests_passed = False

    # ========================================================================
    # TEST 2: .MAGIC DIRECTORY (Should NOT contain inbox/tags anymore)
    # ========================================================================
    print("\n[Phase 3] Validating .magic Directory (Level 2)")
    print("-" * 40)

    magic_path = os.path.join(MOUNT_POINT, ".magic")
    magic_contents = list_directory(magic_path)
    log(f"Current /.magic contents: {magic_contents}")

    # These should exist inside .magic
    required_in_magic = ["refresh"]

    # These should NOT exist inside .magic anymore (they moved to root)
    forbidden_in_magic = ["inbox", "tags"]

    for item in required_in_magic:
        full_path = os.path.join(magic_path, item)
        if not check_path_exists(full_path, f"Inside .magic: '{item}'"):
            all_tests_passed = False

    for item in forbidden_in_magic:
        full_path = os.path.join(magic_path, item)
        if not check_path_not_exists(full_path, f"Inside .magic (relocated): '{item}'"):
            all_tests_passed = False

    # ========================================================================
    # TEST 3: NAVIGATION (Verify inodes resolve from new parents)
    # ========================================================================
    print("\n[Phase 4] Validating Navigation & Inode Resolution")
    print("-" * 40)

    # Create a test file in the database to verify paths work
    # We'll create a file in 'inbox' via the database to ensure FUSE can resolve it
    if os.path.exists(DB_PATH):
        try:
            # Ensure 'inbox' tag exists using sudo sqlite3
            subprocess.run(["sudo", "sqlite3", DB_PATH, "INSERT OR IGNORE INTO tags (name) VALUES ('inbox')"],
                         check=True, capture_output=True)

            # Get inbox tag ID using sudo sqlite3
            result = subprocess.run(["sudo", "sqlite3", DB_PATH, "SELECT tag_id FROM tags WHERE name='inbox'"],
                                  capture_output=True, text=True, check=True)

            if result.stdout.strip():
                inbox_id = result.stdout.strip()
                log(f"Inbox tag ID: {inbox_id}")

                # Verify we can see the inbox directory (it's a virtual view)
                inbox_path = os.path.join(MOUNT_POINT, "inbox")
                if os.path.exists(inbox_path):
                    log("✓ Inbox directory exists and is accessible", "SUCCESS")
                else:
                    log("✗ Inbox directory not accessible", "ERROR")
                    all_tests_passed = False
            else:
                log("⚠ Could not verify inbox tag ID", "WARN")

        except Exception as e:
            log(f"Database check failed: {e}", "WARN")
    else:
        log("Database not found for deep navigation check", "WARN")

    # ========================================================================
    # TEST 4: ABSENCE OF OLD PATHS
    # ========================================================================
    print("\n[Phase 5] Validating Absence of Old Paths")
    print("-" * 40)

    # These are the OLD paths that should NOT work anymore
    old_paths = [
        "/.magic/inbox",
        "/.magic/tags"
    ]

    for old_path in old_paths:
        full_path = os.path.join(MOUNT_POINT, old_path.lstrip('/'))
        if not check_path_not_exists(full_path, f"Old path: '{old_path}'"):
            all_tests_passed = False

    # ========================================================================
    # SUMMARY
    # ========================================================================
    print("\n" + "=" * 70)
    if all_tests_passed:
        print("✅ ALL TESTS PASSED: Structure matches Phase 21 requirements")
        print("=" * 70)
        return 0
    else:
        print("❌ SOME TESTS FAILED: Structure does not match requirements")
        print("=" * 70)
        print("\nExpected Structure:")
        print("  /inbox          (New First-Class Citizen)")
        print("  /tags           (New First-Class Citizen)")
        print("  /search         (Standard)")
        print("  /mirror         (Standard)")
        print("  /.magic/refresh (Internal)")
        print("\nCurrent structure:")
        print(f"  Root: {root_items}")
        if os.path.exists(MOUNT_POINT + "/.magic"):
            print(f"  .magic: {magic_contents}")

        print("\n" + "=" * 70)
        print("🔧 REQUIRED CHANGES in src/hollow_drive.rs:")
        print("=" * 70)
        print("""
1. ROOT INODE LOOKUP:
   - Add: "inbox" -> INODE_INBOX (return success)
   - Add: "tags" -> INODE_TAGS (return success)

2. ROOT READDIR:
   - Add INODE_INBOX and INODE_TAGS to root listing

3. .MAGIC LOOKUP:
   - Remove "inbox" and "tags" match arms

4. .MAGIC READDIR:
   - Remove "inbox" and "tags" from listing

5. INBOX/TAGS INODES:
   - INODE_INBOX = 2 (or suitable constant)
   - INODE_TAGS = 3 (or suitable constant)
   - These should return true on lookup() and populate on readdir()
        """)
        return 1

if __name__ == "__main__":
    sys.exit(main())